// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use host::Host;
use moor_values::{v_float, v_int, v_list, v_map, v_none, v_objid, v_string, Var, Variant};
use neon::prelude::*;
use once_cell::sync::OnceCell;
use tokio::runtime::Runtime;

mod connection;
mod host;

fn var_to_js_value<'a, C: Context<'a>>(cx: &mut C, v: &Var) -> NeonResult<Handle<'a, JsValue>> {
    match v.variant() {
        Variant::None => Ok(cx.undefined().upcast()),
        Variant::Obj(i) => {
            let oid = i.id().0;
            let obj = cx.empty_object();
            let oid = cx.number(oid as f64);
            obj.set(cx, "oid", oid)?;
            Ok(obj.upcast())
        }
        Variant::Int(i) => Ok(cx.number(*i as f64).upcast()),
        Variant::Float(f) => Ok(cx.number(*f).upcast()),
        Variant::List(l) => {
            let arr = cx.empty_array();
            for (i, v) in l.iter().enumerate() {
                let js_value = var_to_js_value(cx, &v)?;
                arr.set(cx, i as u32, js_value)?;
            }
            Ok(arr.upcast())
        }
        Variant::Str(s) => Ok(cx.string(s.as_string()).upcast()),
        Variant::Map(m) => {
            // JavaScript objects cannot have non-string keys, but MOO's can.
            // So we'll represent things as a binary-sorted vector as such:
            // { map: [ [key, value], [key, value], ... ] }
            let map = cx.empty_object();
            let arr = cx.empty_array();
            for (i, (k, v)) in m.iter().enumerate() {
                let key = var_to_js_value(cx, &k)?;
                let value = var_to_js_value(cx, &v)?;
                let pair = cx.empty_array();
                pair.set(cx, 0, key)?;
                pair.set(cx, 1, value)?;
                arr.set(cx, i as u32, pair)?;
            }
            map.set(cx, "map", arr)?;
            Ok(map.upcast())
        }
        Variant::Err(e) => {
            // { error: "E_INVIND" }
            let obj = cx.empty_object();
            let e_name = cx.string(e.name());
            obj.set(cx, "error", e_name)?;
            let e_message = cx.string(e.message());
            obj.set(cx, "message", e_message)?;
            Ok(obj.upcast())
        }
        Variant::Flyweight(f) => {
            // Flyweight is somewhat analogous in structure to an XML element.
            // it has attributes (slots), and contents.
            // { slots: { key: value, key: value, ... }, contents: [ ... ] }
            let flyweight = cx.empty_object();
            let slots = cx.empty_object();
            for (k, v) in f.slots() {
                let key = cx.string(k.to_string());
                let value = var_to_js_value(cx, &v)?;
                slots.set(cx, key, value)?;
            }
            flyweight.set(cx, "slots", slots)?;
            let contents = cx.empty_array();
            for (i, v) in f.contents().iter().enumerate() {
                let js_value = var_to_js_value(cx, &v)?;
                contents.set(cx, i as u32, js_value)?;
            }
            flyweight.set(cx, "contents", contents)?;
            Ok(flyweight.upcast())
        }
    }
}

fn js_value_to_var<'a, C: Context<'a>>(cx: &mut C, v: Handle<'a, JsValue>) -> NeonResult<Var> {
    if v.is_a::<JsUndefined, _>(cx) {
        Ok(v_none())
    } else if v.is_a::<JsNumber, _>(cx) {
        let n = v.downcast::<JsNumber, _>(cx).or_throw(cx)?;
        let n = n.value(cx);
        if n.fract() == 0.0 {
            Ok(v_int(n as i64))
        } else {
            Ok(v_float(n))
        }
    } else if v.is_a::<JsString, _>(cx) {
        let s = v.downcast::<JsString, _>(cx).or_throw(cx)?;
        let s = s.value(cx);
        Ok(v_string(s))
    } else if v.is_a::<JsObject, _>(cx) {
        let obj = v.downcast::<JsObject, _>(cx).or_throw(cx)?;
        let map: Handle<JsValue> = obj.get(cx, "map")?;
        if map.is_a::<JsArray, _>(cx) {
            let map = map.downcast::<JsArray, _>(cx).or_throw(cx)?;
            let mut m = Vec::new();
            for i in 0..map.len(cx) {
                let pair: Handle<JsArray> = map.get(cx, i)?;
                let pair = pair.downcast::<JsArray, _>(cx).or_throw(cx)?;
                if pair.len(cx) == 2 {
                    let l = pair.get(cx, 0);
                    let key = js_value_to_var(cx, l?)?;
                    let r = pair.get(cx, 1);
                    let value = js_value_to_var(cx, r?)?;
                    m.push((key, value));
                }
            }
            return Ok(v_map(&m));
        }
        let oid: Handle<JsValue> = obj.get(cx, "oid")?;
        if oid.is_a::<JsNumber, _>(cx) {
            let oid = oid.downcast::<JsNumber, _>(cx).or_throw(cx)?;
            let oid = oid.value(cx) as i32;
            return Ok(v_objid(oid));
        }

        let slots: Handle<JsValue> = obj.get(cx, "slots")?;
        let contents: Handle<JsValue> = obj.get(cx, "contents")?;
        if slots.is_a::<JsObject, _>(cx) && contents.is_a::<JsArray, _>(cx) {
            let slots = slots.downcast::<JsObject, _>(cx).or_throw(cx)?;
            let mut s = Vec::new();
            let prop_names = slots.get_own_property_names(cx)?;
            for i in 0..prop_names.len(cx) {
                let k: Handle<JsValue> = prop_names.get(cx, i)?;
                let v = slots.get(cx, k)?;
                let v = js_value_to_var(cx, v)?;
                if k.is_a::<JsString, _>(cx) {
                    let k = k.downcast::<JsString, _>(cx).or_throw(cx)?;
                    let k = k.value(cx);
                    s.push((k, v));
                }
            }
            let contents = contents.downcast::<JsArray, _>(cx).or_throw(cx)?;
            let mut c = Vec::new();
            for i in 0..contents.len(cx) {
                let js_value = contents.get(cx, i)?;
                let v = js_value_to_var(cx, js_value)?;
                c.push(v);
            }
            return Ok(v_objid(0));
        }

        Ok(v_none())
    } else if v.is_a::<JsArray, _>(cx) {
        let arr = v.downcast::<JsArray, _>(cx).or_throw(cx)?;
        let mut l = Vec::new();
        for i in 0..arr.len(cx) {
            let js_value = arr.get(cx, i)?;
            let v = js_value_to_var(cx, js_value)?;
            l.push(v);
        }
        Ok(v_list(&l))
    } else {
        Ok(v_none())
    }
}

fn runtime<'a, C: Context<'a>>(cx: &mut C) -> NeonResult<&'static Runtime> {
    static RUNTIME: OnceCell<Runtime> = OnceCell::new();

    RUNTIME.get_or_try_init(|| Runtime::new().or_else(|err| cx.throw_error(err.to_string())))
}

#[neon::main]
fn main(mut cx: ModuleContext) -> NeonResult<()> {
    let main_subscriber = tracing_subscriber::fmt()
        .compact()
        .with_ansi(true)
        .with_file(true)
        .with_line_number(true)
        .with_thread_names(true)
        .with_max_level(tracing::Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(main_subscriber)
        .expect("Unable to set configure logging");

    cx.export_function("createHost", Host::create_host)?;
    cx.export_function("attachToDaemon", host::attach_to_daemon)?;
    cx.export_function("listenHostEvents", host::listen_host_events)?;
    cx.export_function("shutdownHost", host::shutdown_host)?;
    cx.export_function("newConnection", connection::new_connection)?;
    cx.export_function("connectionLogin", connection::connection_login)?;
    cx.export_function("connectionCommand", connection::connection_command)?;
    cx.export_function("connectionDisconnect", connection::connection_disconnect)?;
    cx.export_function("welcomeMessage", connection::connection_welcome_message)?;
    cx.export_function("connectionGetPlayer", connection::connection_get_oid)?;
    cx.export_function(
        "connectionIsAuthenticated",
        connection::connection_is_authenticated,
    )?;

    Ok(())
}

#[cfg(test)]
mod test {

    #[test]
    fn test_var_to_js_value() {}
}
