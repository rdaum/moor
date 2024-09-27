// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use strum::EnumCount;
use tracing::{error, warn};

use moor_compiler::offset_for_builtin;
use moor_compiler::program_to_tree;
use moor_compiler::unparse;
use moor_compiler::GlobalName;
use moor_compiler::Program;
use moor_compiler::{compile, to_literal};
use moor_values::model::ObjFlag;
use moor_values::model::VerbDef;
use moor_values::model::WorldStateError;
use moor_values::model::{ArgSpec, VerbArgsSpec};
use moor_values::model::{BinaryType, VerbAttrs, VerbFlag};
use moor_values::model::{HasUuid, Named};
use moor_values::util::BitEnum;
use moor_values::Error::{E_ARGS, E_INVARG, E_INVIND, E_PERM, E_TYPE, E_VERBNF};
use moor_values::List;
use moor_values::Objid;
use moor_values::Symbol;
use moor_values::Variant;
use moor_values::{v_empty_list, v_list, v_none, v_objid, v_str, v_string, Var};
use moor_values::{v_list_iter, Error};
use moor_values::{AsByteBuffer, Sequence};

use crate::bf_declare;
use crate::builtins::BfRet::Ret;
use crate::builtins::{world_state_bf_err, BfCallState, BfErr, BfRet, BuiltinFunction};
use crate::tasks::command_parse::{parse_preposition_spec, preposition_to_string};

// verb_info (obj <object>, str <verb-desc>) ->  {<owner>, <perms>, <names>}
fn bf_verb_info(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };

    if !bf_args
        .world_state
        .valid(obj)
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::Code(E_INVARG));
    }

    let verb_info = match bf_args.args[1].variant() {
        Variant::Str(verb_desc) => bf_args
            .world_state
            .get_verb(
                bf_args.task_perms_who(),
                obj,
                Symbol::mk_case_insensitive(&verb_desc.as_string()),
            )
            .map_err(world_state_bf_err)?,
        Variant::Int(verb_index) => {
            if verb_index < 1 {
                return Err(BfErr::Code(E_INVARG));
            }
            let verb_index = (verb_index as usize) - 1;
            bf_args
                .world_state
                .get_verb_at_index(bf_args.task_perms_who(), obj, verb_index)
                .map_err(world_state_bf_err)?
        }
        _ => {
            return Err(BfErr::Code(E_TYPE));
        }
    };
    let owner = verb_info.owner();
    let perms = verb_info.flags();
    let names = verb_info.names();

    let mut perms_string = String::new();
    if perms.contains(VerbFlag::Read) {
        perms_string.push('r');
    }
    if perms.contains(VerbFlag::Write) {
        perms_string.push('w');
    }
    if perms.contains(VerbFlag::Exec) {
        perms_string.push('x');
    }
    if perms.contains(VerbFlag::Debug) {
        perms_string.push('d');
    }

    // Join names into a single string, this is how MOO presents it.
    let verb_names = names.join(" ");

    let result = v_list(&[v_objid(owner), v_string(perms_string), v_string(verb_names)]);
    Ok(Ret(result))
}
bf_declare!(verb_info, bf_verb_info);

fn get_verbdef(obj: Objid, verbspec: Var, bf_args: &BfCallState<'_>) -> Result<VerbDef, BfErr> {
    let verbspec_result = match verbspec.variant() {
        Variant::Str(verb_desc) => {
            let verb_desc = Symbol::mk_case_insensitive(&verb_desc.as_string());
            bf_args
                .world_state
                .get_verb(bf_args.task_perms_who(), obj, verb_desc)
        }
        Variant::Int(verb_index) => {
            if verb_index < 1 {
                return Err(BfErr::Code(E_INVARG));
            }
            let verb_index = (verb_index as usize) - 1;
            bf_args
                .world_state
                .get_verb_at_index(bf_args.task_perms_who(), obj, verb_index)
        }
        _ => return Err(BfErr::Code(E_TYPE)),
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
        return Err(E_INVARG);
    }
    match (
        info.index(0)?.variant(),
        info.index(1)?.variant(),
        info.index(2)?.variant(),
    ) {
        (Variant::Obj(owner), Variant::Str(perms_str), Variant::Str(names)) => {
            let mut perms = BitEnum::new();
            for c in perms_str.as_string().chars() {
                match c {
                    'r' => perms |= VerbFlag::Read,
                    'w' => perms |= VerbFlag::Write,
                    'x' => perms |= VerbFlag::Exec,
                    'd' => perms |= VerbFlag::Debug,
                    _ => return Err(E_INVARG),
                }
            }

            // Split the names string into a list of symbols
            let name_strings = names
                .as_string()
                .split(' ')
                .map(Symbol::mk_case_insensitive)
                .collect::<Vec<_>>();

            Ok(VerbAttrs {
                definer: None,
                owner: Some(owner),
                names: Some(name_strings),
                flags: Some(perms),
                args_spec: None,
                binary_type: None,
                binary: None,
            })
        }
        _ => Err(E_INVARG),
    }
}

// set_verb_info (obj <object>, str <verb-desc>, list <info>) => none
fn bf_set_verb_info(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 3 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };
    let Variant::List(info) = bf_args.args[2].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };
    if info.len() != 3 {
        return Err(BfErr::Code(E_ARGS));
    }
    let update_attrs = parse_verb_info(&info).map_err(BfErr::Code)?;

    if !bf_args
        .world_state
        .valid(obj)
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::Code(E_INVARG));
    }

    match bf_args.args[1].variant() {
        Variant::Str(verb_name) => {
            bf_args
                .world_state
                .update_verb(
                    bf_args.task_perms_who(),
                    obj,
                    Symbol::mk_case_insensitive(&verb_name.as_string()),
                    update_attrs,
                )
                .map_err(world_state_bf_err)?;
        }
        Variant::Int(verb_index) => {
            if verb_index < 1 {
                return Err(BfErr::Code(E_INVARG));
            }
            let verb_index = (verb_index as usize) - 1;
            bf_args
                .world_state
                .update_verb_at_index(bf_args.task_perms_who(), obj, verb_index, update_attrs)
                .map_err(world_state_bf_err)?;
        }
        _ => return Err(BfErr::Code(E_TYPE)),
    }

    Ok(Ret(v_none()))
}
bf_declare!(set_verb_info, bf_set_verb_info);

fn bf_verb_args(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };
    if !bf_args
        .world_state
        .valid(obj)
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::Code(E_INVARG));
    }

    let verbdef = match get_verbdef(obj, bf_args.args[1].clone(), bf_args) {
        Ok(v) => v,
        Err(e) => return Err(e),
    };
    let args = verbdef.args();

    // Output is {dobj, prep, iobj} as strings
    let result = v_list(&[
        v_str(args.dobj.to_string()),
        v_str(preposition_to_string(&args.prep)),
        v_str(args.iobj.to_string()),
    ]);
    Ok(Ret(result))
}
bf_declare!(verb_args, bf_verb_args);

fn parse_verb_args(verbinfo: &List) -> Result<VerbArgsSpec, Error> {
    if verbinfo.len() != 3 {
        return Err(E_ARGS);
    }
    match (
        verbinfo.index(0)?.variant(),
        verbinfo.index(1)?.variant(),
        verbinfo.index(2)?.variant(),
    ) {
        (Variant::Str(dobj_str), Variant::Str(prep_str), Variant::Str(iobj_str)) => {
            let (Some(dobj), Some(prep), Some(iobj)) = (
                ArgSpec::from_string(&dobj_str.as_string()),
                parse_preposition_spec(&prep_str.as_string()),
                ArgSpec::from_string(&iobj_str.as_string()),
            ) else {
                return Err(E_INVARG);
            };
            Ok(VerbArgsSpec { dobj, prep, iobj })
        }
        _ => Err(E_INVARG),
    }
}

// set_verb_args (obj <object>, str <verb-desc>, list <args>) => none
fn bf_set_verb_args(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 3 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };
    let Variant::List(verbinfo) = bf_args.args[2].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };
    if verbinfo.len() != 3 {
        return Err(BfErr::Code(E_INVARG));
    }
    if !bf_args
        .world_state
        .valid(obj)
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::Code(E_INVARG));
    }

    let args = parse_verb_args(&verbinfo).map_err(BfErr::Code)?;

    let update_attrs = VerbAttrs {
        definer: None,
        owner: None,
        names: None,
        flags: None,
        args_spec: Some(args),
        binary_type: None,
        binary: None,
    };
    match bf_args.args[1].variant() {
        Variant::Str(verb_name) => {
            bf_args
                .world_state
                .update_verb(
                    bf_args.task_perms_who(),
                    obj,
                    Symbol::mk_case_insensitive(&verb_name.as_string()),
                    update_attrs,
                )
                .map_err(world_state_bf_err)?;
        }
        Variant::Int(verb_index) => {
            if verb_index < 1 {
                return Err(BfErr::Code(E_ARGS));
            }
            let verb_index = (verb_index as usize) - 1;
            bf_args
                .world_state
                .update_verb_at_index(bf_args.task_perms_who(), obj, verb_index, update_attrs)
                .map_err(world_state_bf_err)?;
        }
        _ => return Err(BfErr::Code(E_TYPE)),
    }
    Ok(Ret(v_none()))
}
bf_declare!(set_verb_args, bf_set_verb_args);

fn bf_verb_code(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    //verb_code (obj object, str verb-desc [, fully-paren [, indent]]) => list
    if bf_args.args.len() < 2 || bf_args.args.len() > 4 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };
    if !bf_args
        .world_state
        .valid(obj)
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
    let verbdef = match get_verbdef(obj, bf_args.args[1].clone(), bf_args) {
        Ok(v) => v,
        Err(e) => return Err(e),
    };

    // If the verb is not binary type MOO, we don't support decompilation or listing
    // of it yet.
    if verbdef.binary_type() != BinaryType::LambdaMoo18X {
        warn!(object=?bf_args.args[0], verb=?bf_args.args[1], binary_type=?verbdef.binary_type(), 
            "verb_code: verb is not binary type MOO");
        return Err(BfErr::Code(E_TYPE));
    }

    // TODO: bf_verbs: fully-paren and indent options. For now we ignore these.

    // Retrieve the binary for the verb.
    let verb_info = bf_args
        .world_state
        .retrieve_verb(bf_args.task_perms_who(), obj, verbdef.uuid())
        .map_err(world_state_bf_err)?;

    // If the binary is empty, just return empty rather than try to decode it.
    if verb_info.0.is_empty() {
        return Ok(Ret(v_empty_list()));
    }

    // Decode.
    let program = Program::from_bytes(verb_info.0).map_err(|_| {
        error!(object=?bf_args.args[0], verb=?bf_args.args[1], "verb_code: verb program could not be decoded");
        BfErr::Code(E_INVARG)
    })?;
    let decompiled = match program_to_tree(&program) {
        Ok(decompiled) => decompiled,
        Err(e) => {
            warn!(object=?bf_args.args[0], verb=?bf_args.args[1], error = ?e,
            binary_type=?verbdef.binary_type(), "verb_code: verb program could not be decompiled");
            return Err(BfErr::Code(E_INVARG));
        }
    };

    let unparsed = match unparse(&decompiled) {
        Ok(unparsed) => unparsed,
        Err(e) => {
            warn!(object=?bf_args.args[0], verb=?bf_args.args[1], error = ?e, 
                binary_type=?verbdef.binary_type(), 
            "verb_code: verb program could not be unparsed");
            return Err(BfErr::Code(E_INVARG));
        }
    };
    Ok(Ret(v_list_iter(unparsed.iter().map(|s| v_str(s)))))
}
bf_declare!(verb_code, bf_verb_code);

// Function: list set_verb_code (obj object, str verb-desc, list code)
fn bf_set_verb_code(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    //set_verb_code (obj object, str verb-desc, list code) => none
    if bf_args.args.len() != 3 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };
    if !bf_args
        .world_state
        .valid(obj)
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

    let verbdef = match get_verbdef(obj, bf_args.args[1].clone(), bf_args) {
        Ok(v) => v,
        Err(e) => return Err(e),
    };

    // Right now set_verb_code is going to always compile to LambdaMOO 1.8.x. binary type.
    let binary_type = BinaryType::LambdaMoo18X;
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
        code_string.push_str(&line.as_string());
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
            return Ok(Ret(v_list(&[v_str(e.to_string().as_str())])));
        }
    };
    // Now we have a program, we need to encode it.
    let binary = program
        .with_byte_buffer(|d| Vec::from(d))
        .expect("Failed to encode program byte stream");
    // Now we can update the verb.
    let update_attrs = VerbAttrs {
        definer: None,
        owner: None,
        names: None,
        flags: None,
        args_spec: None,
        binary_type: Some(binary_type),
        binary: Some(binary),
    };
    bf_args
        .world_state
        .update_verb_with_id(bf_args.task_perms_who(), obj, verbdef.uuid(), update_attrs)
        .map_err(world_state_bf_err)?;
    Ok(Ret(v_none()))
}
bf_declare!(set_verb_code, bf_set_verb_code);

// Function: none add_verb (obj object, list info, list args)
fn bf_add_verb(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 3 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };
    let Variant::List(info) = bf_args.args[1].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };
    let Variant::List(args) = bf_args.args[2].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };
    if !bf_args
        .world_state
        .valid(obj)
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
    let verbargs = parse_verb_args(&args).map_err(BfErr::Code)?;
    let verbinfo = parse_verb_info(&info).map_err(BfErr::Code)?;

    bf_args
        .world_state
        .add_verb(
            bf_args.task_perms_who(),
            obj,
            verbinfo.names.unwrap(),
            verbinfo.owner.unwrap(),
            verbinfo.flags.unwrap(),
            verbargs,
            Vec::new(),
            BinaryType::LambdaMoo18X,
        )
        .map_err(world_state_bf_err)?;

    Ok(Ret(v_none()))
}
bf_declare!(add_verb, bf_add_verb);

//Function: none delete_verb (obj object, str verb-desc)
fn bf_delete_verb(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };
    if !bf_args
        .world_state
        .valid(obj)
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

    let verbdef = match get_verbdef(obj, bf_args.args[1].clone(), bf_args) {
        Ok(v) => v,
        Err(e) => return Err(e),
    };

    bf_args
        .world_state
        .remove_verb(bf_args.task_perms_who(), obj, verbdef.uuid())
        .map_err(world_state_bf_err)?;

    Ok(Ret(v_none()))
}
bf_declare!(delete_verb, bf_delete_verb);

// Syntax:  disassemble (obj <object>, str <verb-desc>)   => list
//
// Returns a (longish) list of strings giving a listing of the server's internal ``compiled'' form of the verb as specified by <verb-desc>
// on <object>.  This format is not documented and may indeed change from release to release, but some programmers may nonetheless find
// the output of `disassemble()' interesting to peruse as a way to gain a deeper appreciation of how the server works.
fn bf_disassemble(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };
    if !bf_args
        .world_state
        .valid(obj)
        .map_err(world_state_bf_err)?
    {
        return Err(BfErr::Code(E_INVARG));
    }

    let verbdef = match get_verbdef(obj, bf_args.args[1].clone(), bf_args) {
        Ok(v) => v,
        Err(e) => return Err(e),
    };

    if verbdef.binary_type() != BinaryType::LambdaMoo18X {
        warn!(object=?bf_args.args[0], verb=?bf_args.args[1], binary_type=?verbdef.binary_type(),
            "disassemble: verb is not binary type MOO");
        return Err(BfErr::Code(E_TYPE));
    }

    let (binary, _) = bf_args
        .world_state
        .retrieve_verb(bf_args.task_perms_who(), obj, verbdef.uuid())
        .map_err(world_state_bf_err)?;

    if binary.is_empty() {
        return Ok(Ret(v_empty_list()));
    }

    let program = Program::from_bytes(binary).map_err(|_| {
        error!(object=?bf_args.args[0], verb=?bf_args.args[1], "disassemble: verb program could not be decoded");
        BfErr::Code(E_INVARG)
    })?;

    // The output of disassemble is a list of strings, one for each instruction in the verb's program.
    // But we also want some basic information on # of labels. Fork vectors. Jazz like that.
    // But I'll just keep it simple for now.

    let mut disassembly = Vec::new();
    // Write literals indexed by their offset #
    disassembly.push(v_str("LITERALS:"));
    for (i, l) in program.literals.iter().enumerate() {
        disassembly.push(v_string(format!("{: >3}: {}", i, to_literal(l))));
    }

    // Write jump labels indexed by their offset & showing position & optional name
    disassembly.push(v_str("JUMP LABELS:"));
    for (i, l) in program.jump_labels.iter().enumerate() {
        if i < GlobalName::COUNT {
            continue;
        }
        let name_of = match &l.name {
            Some(name) => format!(" ({})", program.var_names.name_of(name).unwrap()),
            None => "".to_string(),
        };
        disassembly.push(v_string(format!("{: >3}: {}{}", i, l.position.0, name_of)));
    }

    // Write variable names indexed by their offset
    disassembly.push(v_str("VARIABLES:"));
    for (i, v) in program.var_names.symbols().iter().enumerate() {
        disassembly.push(v_string(format!("{: >3}: {}", i, v)));
    }

    // TODO: dump fork vectors in program disassembly stream

    // Display main vector (program); opcodes are indexed by their offset
    disassembly.push(v_str("OPCODES:"));
    for (i, op) in program.main_vector.iter().enumerate() {
        let mut line_no_string = String::new();
        let mut last_line_no = 0;
        for (pc, line_no) in &program.line_number_spans {
            if *pc == i {
                line_no_string = format!("\t\t(line {})", last_line_no);
                break;
            }
            last_line_no = *line_no;
        }
        disassembly.push(v_string(format!("{: >3}: {:?}{}", i, op, line_no_string)));
    }

    Ok(Ret(v_list(&disassembly)))
}
bf_declare!(disassemble, bf_disassemble);

pub(crate) fn register_bf_verbs(builtins: &mut [Box<dyn BuiltinFunction>]) {
    builtins[offset_for_builtin("verb_info")] = Box::new(BfVerbInfo {});
    builtins[offset_for_builtin("set_verb_info")] = Box::new(BfSetVerbInfo {});
    builtins[offset_for_builtin("verb_args")] = Box::new(BfVerbArgs {});
    builtins[offset_for_builtin("set_verb_args")] = Box::new(BfSetVerbArgs {});
    builtins[offset_for_builtin("verb_code")] = Box::new(BfVerbCode {});
    builtins[offset_for_builtin("set_verb_code")] = Box::new(BfSetVerbCode {});
    builtins[offset_for_builtin("add_verb")] = Box::new(BfAddVerb {});
    builtins[offset_for_builtin("delete_verb")] = Box::new(BfDeleteVerb {});
    builtins[offset_for_builtin("disassemble")] = Box::new(BfDisassemble {});
}
