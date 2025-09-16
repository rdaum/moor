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

//! Builtin functions for verb manipulation and introspection.

use strum::EnumCount;
use tracing::{error, warn};

use crate::task_context::{with_current_transaction, with_current_transaction_mut};
use crate::vm::builtins::BfRet::{Ret, RetNil};
use crate::vm::builtins::{BfCallState, BfErr, BfRet, BuiltinFunction, world_state_bf_err};
use moor_common::model::WorldStateError;
use moor_common::model::{ArgSpec, VerbArgsSpec};
use moor_common::model::{HasUuid, Named};
use moor_common::model::{ObjFlag, verb_perms_string};
use moor_common::model::{VerbAttrs, VerbFlag};
use moor_common::model::{VerbDef, parse_preposition_spec, preposition_to_string};
use moor_common::util::BitEnum;
use moor_compiler::Program;
use moor_compiler::offset_for_builtin;
use moor_compiler::program_to_tree;
use moor_compiler::unparse;
use moor_compiler::{compile, to_literal};
use moor_var::Obj;
use moor_var::Sequence;
use moor_var::Symbol;
use moor_var::Variant;
use moor_var::program::ProgramType;
use moor_var::program::names::GlobalName;
use moor_var::{E_ARGS, E_INVARG, E_INVIND, E_PERM, E_TYPE, E_VERBNF};
use moor_var::{Error, v_list_iter};
use moor_var::{List, v_bool};
use moor_var::{Var, v_empty_list, v_list, v_obj, v_str, v_string};

/// MOO: `list verb_info(obj object, str|int verb_desc)`
/// Returns information about a verb as `{owner, perms, names}`.
fn bf_verb_info(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::Code(E_TYPE));
    };

    if !with_current_transaction(|world_state| world_state.valid(&obj))
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::Code(E_INVARG));
    }

    let verb_info = match bf_args.args[1].variant() {
        Variant::Int(verb_index) => {
            if *verb_index < 1 {
                return Err(BfErr::Code(E_INVARG));
            }
            let verb_index = (*verb_index as usize) - 1;
            with_current_transaction(|world_state| {
                world_state.get_verb_at_index(&bf_args.task_perms_who(), &obj, verb_index)
            })
            .map_err(world_state_bf_err)?
        }
        _ => {
            let Ok(verb_name) = bf_args.args[1].as_symbol() else {
                return Err(BfErr::Code(E_TYPE));
            };

            with_current_transaction(|world_state| {
                world_state.get_verb(&bf_args.task_perms_who(), &obj, verb_name)
            })
            .map_err(world_state_bf_err)?
        }
    };

    let owner = verb_info.owner();
    let perms = verb_info.flags();
    let names = verb_info.names();

    let perms_string = verb_perms_string(perms);

    // Join names into a single string, this is how MOO presents it.
    let verb_names = names
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .join(" ");

    let result = v_list(&[v_obj(owner), v_string(perms_string), v_string(verb_names)]);
    Ok(Ret(result))
}

fn get_verbdef(obj: &Obj, verbspec: Var, bf_args: &BfCallState<'_>) -> Result<VerbDef, BfErr> {
    let verbspec_result = match verbspec.variant() {
        Variant::Int(verb_index) => {
            if *verb_index < 1 {
                return Err(BfErr::Code(E_INVARG));
            }
            let verb_index = (*verb_index as usize) - 1;
            with_current_transaction(|world_state| {
                world_state.get_verb_at_index(&bf_args.task_perms_who(), obj, verb_index)
            })
        }
        _ => {
            let Ok(verb_desc) = verbspec.as_symbol() else {
                return Err(BfErr::Code(E_TYPE));
            };

            with_current_transaction(|world_state| {
                world_state.get_verb(&bf_args.task_perms_who(), obj, verb_desc)
            })
        }
    };
    match verbspec_result {
        Ok(vs) => Ok(vs),
        Err(WorldStateError::VerbNotFound(_, _)) => Err(BfErr::Code(E_VERBNF)),
        Err(e) => {
            error!("get_verbdef: unexpected error: {:?}", e);
            Err(BfErr::Code(E_INVIND))
        }
    }
}

fn parse_verb_info(info: &List) -> Result<VerbAttrs, Error> {
    if info.len() != 3 {
        return Err(E_INVARG.msg("verb_info requires 3 elements"));
    }
    match (
        info.index(0)?.variant(),
        info.index(1)?.variant(),
        info.index(2)?.variant(),
    ) {
        (Variant::Obj(owner), Variant::Str(perms_str), Variant::Str(names)) => {
            let mut perms = BitEnum::new();
            for c in perms_str.as_str().chars() {
                match c {
                    'r' => perms |= VerbFlag::Read,
                    'w' => perms |= VerbFlag::Write,
                    'x' => perms |= VerbFlag::Exec,
                    'd' => perms |= VerbFlag::Debug,
                    _ => return Err(E_INVARG.msg("Invalid verb permissions")),
                }
            }

            // Split the names string into a list of symbols
            let name_strings = names
                .as_str()
                .split(' ')
                .map(Symbol::mk)
                .collect::<Vec<_>>();

            Ok(VerbAttrs {
                definer: None,
                owner: Some(*owner),
                names: Some(name_strings),
                flags: Some(perms),
                args_spec: None,
                program: None,
            })
        }
        _ => Err(E_INVARG.msg("Invalid verb info")),
    }
}

/// MOO: `none set_verb_info(obj object, str|int verb_desc, list info)`
/// Sets verb information from a `{owner, perms, names}` list.
fn bf_set_verb_info(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 3 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::Code(E_TYPE));
    };
    let Some(info) = bf_args.args[2].as_list() else {
        return Err(BfErr::Code(E_TYPE));
    };
    if info.len() != 3 {
        return Err(BfErr::Code(E_ARGS));
    }
    let update_attrs = parse_verb_info(info).map_err(BfErr::ErrValue)?;

    if !with_current_transaction(|world_state| world_state.valid(&obj))
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::Code(E_INVARG));
    }

    match bf_args.args[1].variant() {
        Variant::Int(verb_index) => {
            if *verb_index < 1 {
                return Err(BfErr::Code(E_INVARG));
            }
            let verb_index = (*verb_index as usize) - 1;
            with_current_transaction_mut(|world_state| {
                world_state.update_verb_at_index(
                    &bf_args.task_perms_who(),
                    &obj,
                    verb_index,
                    update_attrs,
                )
            })
            .map_err(world_state_bf_err)?;
        }
        _ => {
            let Ok(verb_name) = bf_args.args[1].as_symbol() else {
                return Err(BfErr::Code(E_TYPE));
            };
            with_current_transaction_mut(|world_state| {
                world_state.update_verb(&bf_args.task_perms_who(), &obj, verb_name, update_attrs)
            })
            .map_err(world_state_bf_err)?;
        }
    }

    Ok(RetNil)
}

/// MOO: `list verb_args(obj object, str|int verb_desc)`
/// Returns verb argument specification as `{dobj, prep, iobj}`.
fn bf_verb_args(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::Code(E_TYPE));
    };
    if !with_current_transaction(|world_state| world_state.valid(&obj))
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::Code(E_INVARG));
    }

    let verbdef = get_verbdef(&obj, bf_args.args[1].clone(), bf_args)?;
    let args = verbdef.args();

    // Output is {dobj, prep, iobj} as strings
    let result = v_list(&[
        v_str(args.dobj.to_string()),
        v_str(preposition_to_string(&args.prep)),
        v_str(args.iobj.to_string()),
    ]);
    Ok(Ret(result))
}

fn parse_verb_args(verbinfo: &List) -> Result<VerbArgsSpec, Error> {
    if verbinfo.len() != 3 {
        return Err(E_ARGS.msg("verb_args requires 3 elements"));
    }
    match (
        verbinfo.index(0)?.variant(),
        verbinfo.index(1)?.variant(),
        verbinfo.index(2)?.variant(),
    ) {
        (Variant::Str(dobj_str), Variant::Str(prep_str), Variant::Str(iobj_str)) => {
            let (Some(dobj), Some(prep), Some(iobj)) = (
                ArgSpec::from_string(dobj_str.as_str()),
                parse_preposition_spec(prep_str.as_str()),
                ArgSpec::from_string(iobj_str.as_str()),
            ) else {
                return Err(E_INVARG.msg("Invalid verb args"));
            };
            Ok(VerbArgsSpec { dobj, prep, iobj })
        }
        _ => Err(E_INVARG.msg("Invalid verb args")),
    }
}

/// MOO: `none set_verb_args(obj object, str|int verb_desc, list args)`
/// Sets verb argument specification from a `{dobj, prep, iobj}` list.
fn bf_set_verb_args(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 3 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::Code(E_TYPE));
    };
    let Some(verbinfo) = bf_args.args[2].as_list() else {
        return Err(BfErr::Code(E_TYPE));
    };
    if verbinfo.len() != 3 {
        return Err(BfErr::Code(E_INVARG));
    }
    if !with_current_transaction(|world_state| world_state.valid(&obj))
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::Code(E_INVARG));
    }

    let args = parse_verb_args(verbinfo).map_err(BfErr::ErrValue)?;

    let update_attrs = VerbAttrs {
        definer: None,
        owner: None,
        names: None,
        flags: None,
        args_spec: Some(args),
        program: None,
    };
    match bf_args.args[1].variant() {
        Variant::Int(verb_index) => {
            if *verb_index < 1 {
                return Err(BfErr::Code(E_ARGS));
            }
            let verb_index = (*verb_index as usize) - 1;
            with_current_transaction_mut(|world_state| {
                world_state.update_verb_at_index(
                    &bf_args.task_perms_who(),
                    &obj,
                    verb_index,
                    update_attrs,
                )
            })
            .map_err(world_state_bf_err)?;
        }
        _ => {
            let Ok(verb_name) = bf_args.args[1].as_symbol() else {
                return Err(BfErr::Code(E_TYPE));
            };
            with_current_transaction_mut(|world_state| {
                world_state.update_verb(&bf_args.task_perms_who(), &obj, verb_name, update_attrs)
            })
            .map_err(world_state_bf_err)?;
        }
    }
    Ok(RetNil)
}

/// MOO: `list verb_code(obj object, str|int verb_desc [, bool fully_paren [, bool indent]])`
/// Returns the source code of a verb as a list of strings.
fn bf_verb_code(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() < 2 || bf_args.args.len() > 4 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::Code(E_TYPE));
    };
    if !with_current_transaction(|world_state| world_state.valid(&obj))
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::Code(E_INVARG));
    }

    // Verify caller is a programmer.
    if !bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .flags
        .contains(ObjFlag::Programmer)
    {
        return Err(BfErr::Code(E_PERM));
    }
    let verbdef = get_verbdef(&obj, bf_args.args[1].clone(), bf_args)?;

    // Parse optional fully_paren parameter (defaults to false)
    let fully_paren = if bf_args.args.len() > 2 {
        bf_args.args[2].as_bool().unwrap_or(false)
    } else {
        false
    };

    // Parse optional indent parameter (defaults to true)
    let indent = if bf_args.args.len() > 3 {
        bf_args.args[3].as_bool().unwrap_or(true)
    } else {
        true
    };

    // Retrieve the binary for the verb.
    let verb_info = with_current_transaction(|world_state| {
        world_state.retrieve_verb(&bf_args.task_perms_who(), &obj, verbdef.uuid())
    })
    .map_err(world_state_bf_err)?;

    // If the binary is empty, just return empty rather than try to decode it.
    if verb_info.0.is_empty() {
        return Ok(Ret(v_empty_list()));
    }

    #[allow(irrefutable_let_patterns)]
    let ProgramType::MooR(program) = &verb_info.0 else {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("verb_code: verb program is not Moo"),
        ));
    };
    let decompiled = match program_to_tree(program) {
        Ok(decompiled) => decompiled,
        Err(e) => {
            warn!(object=?bf_args.args[0], verb=?bf_args.args[1], error = ?e, "verb_code: verb program could not be decompiled");
            return Err(BfErr::Code(E_INVARG));
        }
    };

    let unparsed = match unparse(&decompiled, fully_paren, indent) {
        Ok(unparsed) => unparsed,
        Err(e) => {
            warn!(object=?bf_args.args[0], verb=?bf_args.args[1], error = ?e, "verb_code: verb program could not be unparsed");
            return Err(BfErr::Code(E_INVARG));
        }
    };
    Ok(Ret(v_list_iter(unparsed.iter().map(|s| v_str(s)))))
}

/// MOO: `list set_verb_code(obj object, str|int verb_desc, list code)`
/// Sets the source code of a verb. Returns empty list on success, or compilation errors.
fn bf_set_verb_code(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 3 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::Code(E_TYPE));
    };
    if !with_current_transaction(|world_state| world_state.valid(&obj))
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::Code(E_INVARG));
    }

    // Verify caller is a programmer.
    if !bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .flags
        .contains(ObjFlag::Programmer)
    {
        return Err(BfErr::Code(E_PERM));
    }

    let verbdef = get_verbdef(&obj, bf_args.args[1].clone(), bf_args)?;

    // Right now set_verb_code is going to always compile to LambdaMOO 1.8.x. binary type.
    let program_code = match bf_args.args[2].variant() {
        Variant::List(code) => code,
        _ => return Err(BfErr::Code(E_TYPE)),
    };
    // Code should be a list of strings.
    // Which we will join (with linefeeds) into one string.
    let mut code_string = String::new();
    for line in program_code.iter() {
        let line = match line.variant() {
            Variant::Str(line) => line,
            _ => return Err(BfErr::Code(E_TYPE)),
        };
        code_string.push_str(line.as_str());
        code_string.push('\n');
    }
    // Now try to compile...
    let program = match compile(code_string.as_str(), bf_args.config.compile_options()) {
        Ok(program) => program,
        Err(e) => {
            // For set_verb_code(), the result is a list of strings, the error messages generated by the
            // MOO-code compiler during processing of code. If the list is non-empty, then
            // set_verb_code() did not install code; the program associated with the verb in question
            // is unchanged.
            let error_strings = e.to_error_list();
            let error_vars: Vec<Var> = error_strings.iter().map(|s| v_str(s)).collect();
            return Ok(Ret(v_list(&error_vars)));
        }
    };
    // Now we can update the verb.
    let update_attrs = VerbAttrs {
        definer: None,
        owner: None,
        names: None,
        flags: None,
        args_spec: None,
        program: Some(ProgramType::MooR(program)),
    };
    with_current_transaction_mut(|world_state| {
        world_state.update_verb_with_id(
            &bf_args.task_perms_who(),
            &obj,
            verbdef.uuid(),
            update_attrs,
        )
    })
    .map_err(world_state_bf_err)?;
    Ok(RetNil)
}

/// MOO: `none add_verb(obj object, list info, list args)`
/// Adds a new verb with the given info and argument specification.
fn bf_add_verb(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 3 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::Code(E_TYPE));
    };
    let Some(info) = bf_args.args[1].as_list() else {
        return Err(BfErr::Code(E_TYPE));
    };
    let Some(args) = bf_args.args[2].as_list() else {
        return Err(BfErr::Code(E_TYPE));
    };
    if !with_current_transaction(|world_state| world_state.valid(&obj))
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::Code(E_INVARG));
    }

    // Verify caller is a programmer.
    if !bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .flags
        .contains(ObjFlag::Programmer)
    {
        return Err(BfErr::Code(E_PERM));
    }
    let verbargs = parse_verb_args(args).map_err(BfErr::ErrValue)?;
    let verbinfo = parse_verb_info(info).map_err(BfErr::ErrValue)?;

    with_current_transaction_mut(|world_state| {
        world_state.add_verb(
            &bf_args.task_perms_who(),
            &obj,
            verbinfo.names.unwrap(),
            &verbinfo.owner.unwrap(),
            verbinfo.flags.unwrap(),
            verbargs,
            ProgramType::MooR(Program::new()),
        )
    })
    .map_err(world_state_bf_err)?;

    Ok(RetNil)
}

/// MOO: `none delete_verb(obj object, str|int verb_desc)`
/// Removes a verb from an object.
fn bf_delete_verb(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::Code(E_TYPE));
    };
    if !with_current_transaction(|world_state| world_state.valid(&obj))
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::Code(E_INVARG));
    }

    // Verify caller is a programmer.
    if !bf_args
        .task_perms()
        .map_err(world_state_bf_err)?
        .flags
        .contains(ObjFlag::Programmer)
    {
        return Err(BfErr::Code(E_PERM));
    }

    let verbdef = get_verbdef(&obj, bf_args.args[1].clone(), bf_args)?;

    with_current_transaction_mut(|world_state| {
        world_state.remove_verb(&bf_args.task_perms_who(), &obj, verbdef.uuid())
    })
    .map_err(world_state_bf_err)?;

    Ok(RetNil)
}

/// MOO: `list disassemble(obj object, str|int verb_desc)`
/// Returns the internal compiled form of a verb as a list of strings.
/// The format is undocumented and may change between releases.
fn bf_disassemble(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::Code(E_TYPE));
    };
    if !with_current_transaction(|world_state| world_state.valid(&obj))
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::Code(E_INVARG));
    }

    let verbdef = get_verbdef(&obj, bf_args.args[1].clone(), bf_args)?;

    let (program, _) = with_current_transaction(|world_state| {
        world_state.retrieve_verb(&bf_args.task_perms_who(), &obj, verbdef.uuid())
    })
    .map_err(world_state_bf_err)?;

    if program.is_empty() {
        return Ok(Ret(v_empty_list()));
    }

    #[allow(irrefutable_let_patterns)]
    let ProgramType::MooR(program) = &program else {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("disassemble(): verb program is not Moo"),
        ));
    };

    // The output of disassemble is a list of strings, one for each instruction in the verb's program.
    // But we also want some basic information on # of labels. Fork vectors. Jazz like that.
    // But I'll just keep it simple for now.

    let mut disassembly = Vec::new();
    // Write literals indexed by their offset #
    disassembly.push(v_str("LITERALS:"));
    for (i, l) in program.literals().iter().enumerate() {
        disassembly.push(v_string(format!("{: >3}: {}", i, to_literal(l))));
    }

    // Write jump labels indexed by their offset & showing position & optional name
    disassembly.push(v_str("JUMP LABELS:"));
    for (i, l) in program.jump_labels().iter().enumerate() {
        if i < GlobalName::COUNT {
            continue;
        }
        let name_of = match &l.name {
            Some(name) => format!(" ({})", program.var_names().ident_for_name(name).unwrap()),
            None => "".to_string(),
        };
        disassembly.push(v_string(format!("{: >3}: {}{}", i, l.position.0, name_of)));
    }

    // Write variable names indexed by their offset
    disassembly.push(v_str("VARIABLES:"));
    for (i, v) in program.var_names().symbols().iter().enumerate() {
        disassembly.push(v_string(format!("{i: >3}: {v}")));
    }

    // TODO: dump fork vectors in program disassembly stream

    // Display main vector (program); opcodes are indexed by their offset
    disassembly.push(v_str("OPCODES:"));
    for (i, op) in program.main_vector().iter().enumerate() {
        let mut line_no_string = String::new();
        let mut last_line_no = 0;
        for (pc, line_no) in program.line_number_spans().iter() {
            if *pc == i {
                line_no_string = format!("\t\t(line {last_line_no})");
                break;
            }
            last_line_no = *line_no;
        }
        disassembly.push(v_string(format!("{i: >3}: {op:?}{line_no_string}")));
    }

    Ok(Ret(v_list(&disassembly)))
}

/// MOO: `bool|list respond_to(obj object, symbol verb_name)`
/// Returns true if object responds to verb, or `{location, names}` if readable.
fn bf_respond_to(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }

    let Some(obj) = bf_args.args[0].as_object() else {
        return Err(BfErr::Code(E_TYPE));
    };

    if !with_current_transaction(|world_state| world_state.valid(&obj))
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::Code(E_INVARG));
    }

    let name = bf_args.args[1].as_symbol().map_err(BfErr::ErrValue)?;

    let Ok((_, vd)) = with_current_transaction(|world_state| {
        world_state.find_method_verb_on(&bf_args.task_perms_who(), &obj, name)
    }) else {
        return Ok(Ret(v_bool(false)));
    };

    let oflags = with_current_transaction(|world_state| world_state.flags_of(&obj))
        .map_err(world_state_bf_err)?;

    if with_current_transaction(|world_state| world_state.controls(&bf_args.caller_perms(), &obj))
        .map_err(world_state_bf_err)?
        || oflags.contains(ObjFlag::Read)
    {
        let names = v_string(
            vd.names()
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(" "),
        );
        let result = v_list(&[v_obj(vd.location()), names]);
        Ok(Ret(result))
    } else {
        Ok(Ret(v_bool(true)))
    }
}

pub(crate) fn register_bf_verbs(builtins: &mut [Box<BuiltinFunction>]) {
    builtins[offset_for_builtin("verb_info")] = Box::new(bf_verb_info);
    builtins[offset_for_builtin("set_verb_info")] = Box::new(bf_set_verb_info);
    builtins[offset_for_builtin("verb_args")] = Box::new(bf_verb_args);
    builtins[offset_for_builtin("set_verb_args")] = Box::new(bf_set_verb_args);
    builtins[offset_for_builtin("verb_code")] = Box::new(bf_verb_code);
    builtins[offset_for_builtin("set_verb_code")] = Box::new(bf_set_verb_code);
    builtins[offset_for_builtin("add_verb")] = Box::new(bf_add_verb);
    builtins[offset_for_builtin("delete_verb")] = Box::new(bf_delete_verb);
    builtins[offset_for_builtin("disassemble")] = Box::new(bf_disassemble);
    builtins[offset_for_builtin("respond_to")] = Box::new(bf_respond_to);
}
