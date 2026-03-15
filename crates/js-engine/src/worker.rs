// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

//! V8 worker thread. Owns an isolate, accepts dock requests, runs JS verbs,
//! and trampolines back to the kernel for property/verb access.
//!
//! Supports reentrant JS→Moo→JS by accepting both DockRequests and
//! TrampolineResponses on a single input channel.

use std::{collections::HashMap, sync::LazyLock};

use tracing::debug;

use crate::{
    DockRequest, JsError, TrampolineRequest, TrampolineResponse, WorkerInput, marshal,
};

/// Initialize the V8 platform exactly once.
static V8_INIT: LazyLock<()> = LazyLock::new(|| {
    let platform = v8::new_default_platform(0, false).make_shared();
    v8::V8::initialize_platform(platform);
    v8::V8::initialize();
});

// Thread-local trampoline state shared between the worker loop and JS callbacks.
thread_local! {
    static TRAMPOLINE: std::cell::RefCell<TrampolineState> =
        std::cell::RefCell::new(TrampolineState::default());
}

#[derive(Default)]
struct TrampolineState {
    /// Which verb's trampoline_tx to use for the currently executing JS code.
    current_tx: Option<flume::Sender<TrampolineRequest>>,
    /// Counter for assigning unique resolver IDs.
    next_resolver_id: usize,
    /// Pending promise resolvers, keyed by resolver_id.
    pending_resolvers: HashMap<usize, v8::Global<v8::PromiseResolver>>,
}

/// An in-flight JS verb execution tracked by the worker.
struct ActiveVerb {
    promise: v8::Global<v8::Promise>,
    trampoline_tx: flume::Sender<TrampolineRequest>,
}

pub(crate) fn worker_main(worker_rx: flume::Receiver<WorkerInput>) {
    LazyLock::force(&V8_INIT);

    let mut isolate = v8::Isolate::new(v8::CreateParams::default());

    let context = {
        let scope = &mut v8::HandleScope::new(&mut isolate);
        let context = v8::Context::new(scope, Default::default());
        let scope = &mut v8::ContextScope::new(scope, context);

        marshal::install_wrapper_templates(scope);
        install_moor_trampoline(scope);

        v8::Global::new(scope, context)
    };

    debug!("JS worker ready");

    let mut active_verbs: Vec<ActiveVerb> = Vec::new();

    loop {
        // Block waiting for work.
        let input = match worker_rx.recv() {
            Ok(input) => input,
            Err(_) => break, // Channel closed, shutdown.
        };

        let scope = &mut v8::HandleScope::new(&mut isolate);
        let ctx = v8::Local::new(scope, &context);
        let scope = &mut v8::ContextScope::new(scope, ctx);

        // Process the input.
        match input {
            WorkerInput::Dock(dock) => {
                handle_dock(scope, dock, &mut active_verbs);
            }
            WorkerInput::Response {
                resolver_id,
                response,
            } => {
                resolve_trampoline(scope, resolver_id, response);
            }
        }

        // Run microtasks and check for settled promises.
        drain_and_check(scope, &mut active_verbs, &worker_rx);
    }

    debug!("JS worker shutting down");
}

/// Compile and call a new JS verb, adding it to active_verbs.
fn handle_dock(
    scope: &mut v8::HandleScope<'_>,
    dock: DockRequest,
    active_verbs: &mut Vec<ActiveVerb>,
) {
    let DockRequest {
        source,
        this,
        player,
        args,
        trampoline_tx,
    } = dock;

    // Set the current trampoline_tx for JS callbacks.
    TRAMPOLINE.with(|t| t.borrow_mut().current_tx = Some(trampoline_tx.clone()));

    let wrapped = format!(
        "(async function(__moor, __this, __player, __args) {{\n{source}\n}})"
    );

    let v8_source = match v8::String::new(scope, &wrapped) {
        Some(s) => s,
        None => {
            let _ = trampoline_tx.send(TrampolineRequest::Complete(Err(JsError {
                message: "Failed to create V8 string from source".into(),
            })));
            return;
        }
    };

    let script = match v8::Script::compile(scope, v8_source, None) {
        Some(s) => s,
        None => {
            let msg = extract_exception_message(scope);
            let _ = trampoline_tx.send(TrampolineRequest::Complete(Err(JsError {
                message: format!("Compilation error: {msg}"),
            })));
            return;
        }
    };

    let func_val = match script.run(scope) {
        Some(v) => v,
        None => {
            let msg = extract_exception_message(scope);
            let _ = trampoline_tx.send(TrampolineRequest::Complete(Err(JsError {
                message: format!("Failed to evaluate wrapper: {msg}"),
            })));
            return;
        }
    };

    let func = match v8::Local::<v8::Function>::try_from(func_val) {
        Ok(f) => f,
        Err(_) => {
            let _ = trampoline_tx.send(TrampolineRequest::Complete(Err(JsError {
                message: "Wrapper did not evaluate to a function".into(),
            })));
            return;
        }
    };

    // Prepare arguments.
    let global = scope.get_current_context().global(scope);
    let moor_key = v8::String::new(scope, "moor").unwrap();
    let moor_obj = global.get(scope, moor_key.into()).unwrap();

    let this_val = marshal::var_to_v8(scope, &moor_var::v_obj(this));
    let player_val = marshal::var_to_v8(scope, &moor_var::v_obj(player));
    let args_array = v8::Array::new(scope, args.len() as i32);
    for (i, arg) in args.iter().enumerate() {
        let val = marshal::var_to_v8(scope, arg);
        args_array.set_index(scope, i as u32, val);
    }

    let recv = v8::undefined(scope).into();
    let call_args: [v8::Local<v8::Value>; 4] =
        [moor_obj, this_val, player_val, args_array.into()];

    let result = func.call(scope, recv, &call_args);

    match result {
        Some(val) => match v8::Local::<v8::Promise>::try_from(val) {
            Ok(promise) => {
                active_verbs.push(ActiveVerb {
                    promise: v8::Global::new(scope, promise),
                    trampoline_tx,
                });
            }
            Err(_) => {
                // Synchronous return.
                let var = marshal::v8_to_var(scope, val);
                let _ = trampoline_tx.send(TrampolineRequest::Complete(Ok(var)));
            }
        },
        None => {
            let msg = extract_exception_message(scope);
            let _ = trampoline_tx.send(TrampolineRequest::Complete(Err(JsError {
                message: format!("JS execution error: {msg}"),
            })));
        }
    }
}

/// Resolve a pending trampoline promise by resolver_id.
fn resolve_trampoline(
    scope: &mut v8::HandleScope<'_>,
    resolver_id: usize,
    response: TrampolineResponse,
) {
    let resolver_global = TRAMPOLINE.with(|t| t.borrow_mut().pending_resolvers.remove(&resolver_id));

    let Some(resolver_global) = resolver_global else {
        tracing::warn!(resolver_id, "No pending resolver found");
        return;
    };

    let resolver = v8::Local::new(scope, resolver_global);

    match response {
        TrampolineResponse::Value(var) => {
            let val = marshal::var_to_v8(scope, &var);
            resolver.resolve(scope, val);
        }
        TrampolineResponse::Error(err) => {
            let msg = v8::String::new(scope, &err.message).unwrap();
            let exception = v8::Exception::error(scope, msg);
            resolver.reject(scope, exception);
        }
    }
}

/// Run microtask checkpoint, check for settled promises, and keep processing
/// inputs until all active verbs are either settled or waiting for responses.
fn drain_and_check(
    scope: &mut v8::HandleScope<'_>,
    active_verbs: &mut Vec<ActiveVerb>,
    worker_rx: &flume::Receiver<WorkerInput>,
) {
    loop {
        scope.perform_microtask_checkpoint();

        // Check for settled promises.
        let mut any_settled = false;
        active_verbs.retain(|verb| {
            let promise = v8::Local::new(scope, &verb.promise);
            match promise.state() {
                v8::PromiseState::Fulfilled => {
                    let val = promise.result(scope);
                    let var = marshal::v8_to_var(scope, val);
                    let _ = verb.trampoline_tx.send(TrampolineRequest::Complete(Ok(var)));
                    any_settled = true;
                    false
                }
                v8::PromiseState::Rejected => {
                    let val = promise.result(scope);
                    let msg = val.to_rust_string_lossy(scope);
                    let _ = verb
                        .trampoline_tx
                        .send(TrampolineRequest::Complete(Err(JsError { message: msg })));
                    any_settled = true;
                    false
                }
                v8::PromiseState::Pending => true,
            }
        });

        // If nothing settled and there are active verbs pending, we need more
        // input (responses or new docks). Non-blocking check: if there's a
        // message ready, process it and loop. Otherwise return to the outer
        // blocking recv.
        if !any_settled {
            // Try to pick up any immediately available messages (non-blocking).
            match worker_rx.try_recv() {
                Ok(WorkerInput::Dock(dock)) => {
                    handle_dock(scope, dock, active_verbs);
                    continue;
                }
                Ok(WorkerInput::Response {
                    resolver_id,
                    response,
                }) => {
                    resolve_trampoline(scope, resolver_id, response);
                    continue;
                }
                Err(_) => return,
            }
        }
        // If something settled, loop to checkpoint again — settling might have
        // unblocked other continuations.
    }
}

/// Install the `moor` global object with trampoline methods.
/// Called once during context setup.
fn install_moor_trampoline(scope: &mut v8::HandleScope<'_>) {
    let obj = v8::Object::new(scope);

    let call_verb_fn = v8::Function::new(scope, trampoline_call_verb).unwrap();
    let call_verb_key = v8::String::new(scope, "call_verb").unwrap();
    obj.set(scope, call_verb_key.into(), call_verb_fn.into());

    let get_prop_fn = v8::Function::new(scope, trampoline_get_prop).unwrap();
    let get_prop_key = v8::String::new(scope, "get_prop").unwrap();
    obj.set(scope, get_prop_key.into(), get_prop_fn.into());

    let set_prop_fn = v8::Function::new(scope, trampoline_set_prop).unwrap();
    let set_prop_key = v8::String::new(scope, "set_prop").unwrap();
    obj.set(scope, set_prop_key.into(), set_prop_fn.into());

    let global = scope.get_current_context().global(scope);
    let key = v8::String::new(scope, "moor").unwrap();
    global.set(scope, key.into(), obj.into());
}

/// Create a PromiseResolver, assign a resolver_id, stash it, return the promise.
fn create_trampoline_promise<'s>(scope: &mut v8::HandleScope<'s>) -> (usize, v8::Local<'s, v8::Promise>) {
    let resolver = v8::PromiseResolver::new(scope).unwrap();
    let promise = resolver.get_promise(scope);
    let resolver_global = v8::Global::new(scope, resolver);

    let resolver_id = TRAMPOLINE.with(|t| {
        let mut state = t.borrow_mut();
        let id = state.next_resolver_id;
        state.next_resolver_id += 1;
        state.pending_resolvers.insert(id, resolver_global);
        id
    });

    (resolver_id, promise)
}

/// Get the current trampoline_tx from thread-local state.
fn get_current_tx() -> Option<flume::Sender<TrampolineRequest>> {
    TRAMPOLINE.with(|t| t.borrow().current_tx.clone())
}

/// Extract Obj from a V8 value — either a MoorObj wrapper or an integer.
fn v8_to_obj(scope: &mut v8::HandleScope<'_>, val: v8::Local<v8::Value>) -> Option<moor_var::Obj> {
    if let Ok(obj) = v8::Local::<v8::Object>::try_from(val) {
        if let Some(var) = marshal::extract_var_from_internal(scope, obj) {
            if let moor_var::Variant::Obj(o) = var.variant() {
                return Some(o);
            }
        }
    }
    if val.is_int32() {
        let id = val.int32_value(scope).unwrap_or(0);
        return Some(moor_var::Obj::mk_id(id));
    }
    None
}

// ---------------------------------------------------------------------------
// Trampoline native callbacks
// ---------------------------------------------------------------------------

fn trampoline_call_verb(
    scope: &mut v8::HandleScope<'_>,
    args: v8::FunctionCallbackArguments<'_>,
    mut rv: v8::ReturnValue<'_>,
) {
    let Some(tx) = get_current_tx() else {
        let msg = v8::String::new(scope, "trampoline channel unavailable").unwrap();
        let exc = v8::Exception::error(scope, msg);
        scope.throw_exception(exc);
        return;
    };

    let this_obj = match v8_to_obj(scope, args.get(0)) {
        Some(o) => o,
        None => {
            let msg = v8::String::new(scope, "call_verb: first arg must be an object").unwrap();
            let exc = v8::Exception::type_error(scope, msg);
            scope.throw_exception(exc);
            return;
        }
    };

    let verb_name = args.get(1).to_rust_string_lossy(scope);
    let verb_sym = moor_var::Symbol::mk(&verb_name);

    let mut call_args = Vec::new();
    let args_val = args.get(2);
    if let Ok(arr) = v8::Local::<v8::Array>::try_from(args_val) {
        for i in 0..arr.length() {
            let item = arr.get_index(scope, i).unwrap();
            call_args.push(marshal::v8_to_var(scope, item));
        }
    }

    let (resolver_id, promise) = create_trampoline_promise(scope);

    let _ = tx.send(TrampolineRequest::CallVerb {
        resolver_id,
        this: this_obj,
        verb: verb_sym,
        args: call_args,
    });

    rv.set(promise.into());
}

fn trampoline_get_prop(
    scope: &mut v8::HandleScope<'_>,
    args: v8::FunctionCallbackArguments<'_>,
    mut rv: v8::ReturnValue<'_>,
) {
    let Some(tx) = get_current_tx() else {
        let msg = v8::String::new(scope, "trampoline channel unavailable").unwrap();
        let exc = v8::Exception::error(scope, msg);
        scope.throw_exception(exc);
        return;
    };

    let obj = match v8_to_obj(scope, args.get(0)) {
        Some(o) => o,
        None => {
            let msg = v8::String::new(scope, "get_prop: first arg must be an object").unwrap();
            let exc = v8::Exception::type_error(scope, msg);
            scope.throw_exception(exc);
            return;
        }
    };

    let prop_name = args.get(1).to_rust_string_lossy(scope);
    let prop_sym = moor_var::Symbol::mk(&prop_name);

    let (resolver_id, promise) = create_trampoline_promise(scope);

    let _ = tx.send(TrampolineRequest::GetProp {
        resolver_id,
        obj,
        prop: prop_sym,
    });

    rv.set(promise.into());
}

fn trampoline_set_prop(
    scope: &mut v8::HandleScope<'_>,
    args: v8::FunctionCallbackArguments<'_>,
    mut rv: v8::ReturnValue<'_>,
) {
    let Some(tx) = get_current_tx() else {
        let msg = v8::String::new(scope, "trampoline channel unavailable").unwrap();
        let exc = v8::Exception::error(scope, msg);
        scope.throw_exception(exc);
        return;
    };

    let obj = match v8_to_obj(scope, args.get(0)) {
        Some(o) => o,
        None => {
            let msg = v8::String::new(scope, "set_prop: first arg must be an object").unwrap();
            let exc = v8::Exception::type_error(scope, msg);
            scope.throw_exception(exc);
            return;
        }
    };

    let prop_name = args.get(1).to_rust_string_lossy(scope);
    let prop_sym = moor_var::Symbol::mk(&prop_name);
    let value = marshal::v8_to_var(scope, args.get(2));

    let (resolver_id, promise) = create_trampoline_promise(scope);

    let _ = tx.send(TrampolineRequest::SetProp {
        resolver_id,
        obj,
        prop: prop_sym,
        value,
    });

    rv.set(promise.into());
}

fn extract_exception_message(scope: &mut v8::HandleScope<'_>) -> String {
    let tc = &mut v8::TryCatch::new(scope);
    if let Some(exception) = tc.exception() {
        exception.to_rust_string_lossy(tc)
    } else {
        "Unknown error".to_string()
    }
}
