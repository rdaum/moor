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

use crate::{
    task_context::{with_current_transaction, with_current_transaction_mut},
    vm::builtins::{
        BfCallState, BfErr, BfRet,
        BfRet::{Ret, RetNil},
        BuiltinFunction, DiagnosticOutput, parse_diagnostic_options, world_state_bf_err,
    },
};
use moor_common::{
    matching::{all_prepositions, get_preposition_forms},
    model::{
        ArgSpec, CompileContext, CompileError, HasUuid, Named, ObjFlag, ParseErrorDetails,
        VerbArgsSpec, VerbAttrs, VerbDef, VerbFlag, WorldStateError, parse_preposition_spec,
        preposition_to_string, verb_perms_string,
    },
    util::BitEnum,
};
use moor_compiler::{
    Program, compile, compile_error_to_map, format_compile_error, offset_for_builtin,
    program_to_tree, to_literal, unparse,
};
use moor_var::{
    E_ARGS, E_INVARG, E_INVIND, E_PERM, E_TYPE, E_VERBNF, Error, IndexMode, List, Obj, Sequence,
    Symbol, Var, Variant,
    program::{ProgramType, names::GlobalName},
    v_bool, v_empty_list, v_list, v_list_iter, v_obj, v_str, v_string, v_sym,
};

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
        Err(WorldStateError::VerbNotFound(_, _)) => {
            Err(BfErr::ErrValue(E_VERBNF.with_msg(|| {
                format!("Verb ({}) Not Found", to_literal(&verbspec))
            })))
        }
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

/// MOO: `list set_verb_code(obj object, str|int verb_desc, list code [, int verbosity [, int output_mode]])`
/// Sets the source code of a verb. Returns empty list on success, or compilation errors.
///
/// Arguments controlling compilation error output:
///   - verbosity: Controls error detail level (default: 2)
///     - 0=summary: Brief error message only
///     - 1=context: Message with error location (graphical display when output_mode > 0)
///     - 2=detailed: Message, location, and diagnostic hints
///     - 3=structured map: Returns error data as map for programmatic handling
///   - output_mode: Controls error formatting style (default: 0)
///     - 0=plain text
///     - 1=graphics: Unicode box-drawing characters
///     - 2=graphics+color: Graphics with ANSI color codes
///
/// When verbosity=3, returns a map with diagnostic data instead of formatted strings.
/// Use `format_compile_error()` to format the structured map into human-readable text.
fn bf_set_verb_code(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() < 3 || bf_args.args.len() > 5 {
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

    // Parse optional verbosity (default 2 for set_verb_code) and output_mode (default 0)
    let verbosity = if bf_args.args.len() >= 4 {
        bf_args.args[3].as_integer()
    } else {
        Some(2) // Default to detailed for set_verb_code
    };

    let output_mode = if bf_args.args.len() >= 5 {
        bf_args.args[4].as_integer()
    } else {
        Some(0)
    };

    let diagnostic_output = parse_diagnostic_options(verbosity, output_mode)?;

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
            // For set_verb_code(), the result is either a list of formatted error strings,
            // or a structured error map (if verbosity=3)
            let error_result = match diagnostic_output {
                DiagnosticOutput::Formatted(options) => {
                    let formatted = format_compile_error(&e, Some(code_string.as_str()), options);
                    let error_vars: Vec<Var> = formatted.into_iter().map(v_string).collect();
                    v_list(error_vars.as_slice())
                }
                DiagnosticOutput::Structured => {
                    compile_error_to_map(&e, Some(code_string.as_str()), bf_args.config.symbol_type)
                }
            };
            return Ok(Ret(error_result));
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

/// MOO: `bool|list respond_to(obj|flyweight object, symbol verb_name)`
/// Returns true if object responds to verb, or `{location, names}` if readable.
/// For flyweights, checks the delegate object.
fn bf_respond_to(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }

    // Get the object to check - either directly or from flyweight's delegate
    let obj = if let Some(o) = bf_args.args[0].as_object() {
        o
    } else if let Some(fw) = bf_args.args[0].as_flyweight() {
        *fw.delegate()
    } else {
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

/// MOO: `list format_error(map error [, int verbosity [, int output_mode]])`
/// Formats a structured error map (from set_verb_code or eval with verbosity=3) into human-readable strings.
///
/// Arguments:
///   - error: map with diagnostic data (type, message, line, column, etc.)
///   - verbosity: 0=summary, 1=context, 2=detailed (default: 2)
///   - output_mode: 0=plain text, 1=graphics, 2=graphics+color (default: 0)
///
/// Returns a list of formatted error strings.
fn bf_format_compile_error(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.is_empty() || bf_args.args.len() > 3 {
        return Err(BfErr::Code(E_ARGS));
    }

    // Verify first arg is a map
    if bf_args.args[0].as_map().is_none() {
        return Err(BfErr::Code(E_TYPE));
    }

    // Parse optional verbosity (default 2) and output_mode (default 0)
    let verbosity = if bf_args.args.len() >= 2 {
        bf_args.args[1].as_integer()
    } else {
        Some(2)
    };

    let output_mode = if bf_args.args.len() >= 3 {
        bf_args.args[2].as_integer()
    } else {
        Some(0)
    };

    let diagnostic_output = parse_diagnostic_options(verbosity, output_mode)?;
    let DiagnosticOutput::Formatted(options) = diagnostic_output else {
        return Err(BfErr::Code(E_INVARG));
    };

    // Helper to get a string value from the map by key (handles both symbol and string keys)
    let error_map_var = &bf_args.args[0];
    let mk_key = |key: &str| -> Var {
        if bf_args.config.symbol_type {
            v_sym(Symbol::mk(key))
        } else {
            v_str(key)
        }
    };

    let get_string_field = |key: &str| -> Option<String> {
        error_map_var
            .get(&mk_key(key), IndexMode::ZeroBased)
            .ok()
            .and_then(|v| {
                // Value might be symbol or string
                v.as_symbol()
                    .ok()
                    .map(|s| s.as_string())
                    .or_else(|| v.as_string().map(|s| s.to_string()))
            })
    };

    // Helper to get an integer value from the map by key
    let get_int_field = |key: &str| -> Option<i64> {
        error_map_var
            .get(&mk_key(key), IndexMode::ZeroBased)
            .ok()
            .and_then(|v| v.as_integer())
    };

    // Helper to get a list of strings from the map by key
    let get_string_list_field = |key: &str| -> Vec<String> {
        error_map_var
            .get(&mk_key(key), IndexMode::ZeroBased)
            .ok()
            .and_then(|v| {
                v.as_list().map(|l| {
                    l.iter()
                        .filter_map(|item| item.as_string().map(|s| s.to_string()))
                        .collect()
                })
            })
            .unwrap_or_default()
    };

    // Extract fields from the error map
    let Some(error_type) = get_string_field("type") else {
        return Err(BfErr::Code(E_INVARG));
    };

    if error_type != "parse" {
        // For non-parse errors, just return the message
        let message = get_string_field("message").unwrap_or_else(|| "Unknown error".to_string());
        return Ok(Ret(v_list(&[v_string(message)])));
    }

    // Build ParseError details
    let message = get_string_field("message").unwrap_or_else(|| "Parse error".to_string());
    let line = get_int_field("line").unwrap_or(1) as usize;
    let column = get_int_field("column").unwrap_or(1) as usize;
    let context = get_string_field("context").unwrap_or_default();
    let expected_tokens = get_string_list_field("expected_tokens");
    let notes = get_string_list_field("notes");
    let source = get_string_field("source");

    // Extract span if available
    let span = match (get_int_field("span_start"), get_int_field("span_end")) {
        (Some(start), Some(end)) => Some((start as usize, end as usize)),
        _ => None,
    };

    // Build CompileError and format it
    let error_position = CompileContext {
        line_col: (line, column),
    };

    let details = Box::new(ParseErrorDetails {
        span,
        expected_tokens,
        notes,
    });

    let compile_error = CompileError::ParseError {
        error_position,
        context,
        end_line_col: None,
        message,
        details,
    };

    let formatted = format_compile_error(&compile_error, source.as_deref(), options);
    let error_vars: Vec<Var> = formatted.into_iter().map(v_string).collect();
    Ok(Ret(v_list(error_vars.as_slice())))
}

/// MOO: `list prepositions()`
/// Returns a list of all valid prepositions. Each entry is `{id, short_form, {all_forms}}`.
/// IDs are 1-indexed for MOO compatibility (0 = id 1, 1 = id 2, etc.).
fn bf_prepositions(_bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    let preps = all_prepositions();
    let mut result = Vec::new();

    for (index, prep) in preps.iter().enumerate() {
        // 1-indexed for MOO
        let id = (index + 1) as i64;
        let short_form = v_str(prep.to_string_single());
        let forms = get_preposition_forms(*prep);
        let forms_list = v_list_iter(forms.into_iter().map(v_str));

        result.push(v_list(&[id.into(), short_form, forms_list]));
    }

    Ok(Ret(v_list(&result)))
}

pub(crate) fn register_bf_verbs(builtins: &mut [BuiltinFunction]) {
    builtins[offset_for_builtin("verb_info")] = bf_verb_info;
    builtins[offset_for_builtin("set_verb_info")] = bf_set_verb_info;
    builtins[offset_for_builtin("verb_args")] = bf_verb_args;
    builtins[offset_for_builtin("set_verb_args")] = bf_set_verb_args;
    builtins[offset_for_builtin("verb_code")] = bf_verb_code;
    builtins[offset_for_builtin("set_verb_code")] = bf_set_verb_code;
    builtins[offset_for_builtin("format_compile_error")] = bf_format_compile_error;
    builtins[offset_for_builtin("add_verb")] = bf_add_verb;
    builtins[offset_for_builtin("delete_verb")] = bf_delete_verb;
    builtins[offset_for_builtin("disassemble")] = bf_disassemble;
    builtins[offset_for_builtin("respond_to")] = bf_respond_to;
    builtins[offset_for_builtin("prepositions")] = bf_prepositions;
}
