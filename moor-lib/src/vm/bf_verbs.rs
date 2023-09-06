use std::sync::Arc;

use async_trait::async_trait;
use moor_value::model::defset::HasUuid;
use moor_value::model::objects::ObjFlag;
use moor_value::AsByteBuffer;
use strum::EnumCount;
use tracing::{error, warn};

use moor_value::model::r#match::{ArgSpec, VerbArgsSpec};
use moor_value::model::verbdef::VerbDef;
use moor_value::model::verbs::{BinaryType, VerbAttrs, VerbFlag};
use moor_value::model::WorldStateError;
use moor_value::util::bitenum::BitEnum;
use moor_value::var::error::Error::{E_INVARG, E_INVIND, E_PERM, E_TYPE, E_VERBNF};
use moor_value::var::list::List;
use moor_value::var::objid::Objid;
use moor_value::var::variant::Variant;
use moor_value::var::{v_empty_list, v_list, v_none, v_objid, v_str, v_string, Var};

use crate::bf_declare;
use crate::compiler::builtins::offset_for_builtin;
use crate::compiler::codegen::compile;
use crate::compiler::decompile::program_to_tree;
use crate::compiler::unparse::unparse;
use crate::compiler::GlobalName;
use crate::tasks::command_parse::{parse_preposition_string, preposition_to_string};
use crate::vm::builtin::BfRet::{Error, Ret};
use crate::vm::builtin::{BfCallState, BfRet, BuiltinFunction};
use crate::vm::opcode::Program;
use crate::vm::VM;

// verb_info (obj <object>, str <verb-desc>) ->  {<owner>, <perms>, <names>}
async fn bf_verb_info<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };

    let verb_info = match bf_args.args[1].variant() {
        Variant::Str(verb_desc) => {
            bf_args
                .world_state
                .get_verb(bf_args.task_perms_who(), *obj, verb_desc.as_str())
                .await?
        }
        Variant::Int(verb_index) => {
            let verb_index = *verb_index;
            if verb_index < 1 {
                return Ok(Error(E_INVARG));
            }
            let verb_index = (verb_index as usize) - 1;
            bf_args
                .world_state
                .get_verb_at_index(bf_args.task_perms_who(), *obj, verb_index)
                .await?
        }
        _ => {
            return Ok(Error(E_TYPE));
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

    let result = v_list(vec![
        v_objid(owner),
        v_string(perms_string),
        v_string(verb_names),
    ]);
    Ok(Ret(result))
}
bf_declare!(verb_info, bf_verb_info);

async fn get_verbdef(
    obj: Objid,
    verbspec: Var,
    bf_args: &BfCallState<'_>,
) -> Result<VerbDef, moor_value::var::error::Error> {
    let verbspec_result = match verbspec.variant() {
        Variant::Str(verb_desc) => {
            let verb_desc = verb_desc.as_str();
            bf_args
                .world_state
                .get_verb(bf_args.task_perms_who(), obj, verb_desc)
                .await
        }
        Variant::Int(verb_index) => {
            let verb_index = *verb_index;
            if verb_index < 1 {
                return Err(E_INVARG);
            }
            let verb_index = (verb_index as usize) - 1;
            bf_args
                .world_state
                .get_verb_at_index(bf_args.task_perms_who(), obj, verb_index)
                .await
        }
        _ => return Err(E_TYPE),
    };
    match verbspec_result {
        Ok(vs) => Ok(vs),
        Err(WorldStateError::VerbNotFound(_, _)) => return Err(E_VERBNF),
        Err(e) => {
            error!("get_verbdef: unexpected error: {:?}", e);
            return Err(E_INVIND);
        }
    }
}

fn parse_verb_info(info: &List) -> Result<VerbAttrs, anyhow::Error> {
    if info.len() != 3 {
        return Err(anyhow::anyhow!("Invalid verb info"));
    }
    match (info[0].variant(), info[1].variant(), info[2].variant()) {
        (Variant::Obj(owner), Variant::Str(perms_str), Variant::Str(names)) => {
            let mut perms = BitEnum::new();
            for c in perms_str.as_str().chars() {
                match c {
                    'r' => perms |= VerbFlag::Read,
                    'w' => perms |= VerbFlag::Write,
                    'x' => perms |= VerbFlag::Exec,
                    'd' => perms |= VerbFlag::Debug,
                    _ => return Err(anyhow::anyhow!("Invalid verb info")),
                }
            }

            // Split the names string into a list of strings.
            let name_strings = names
                .as_str()
                .split(' ')
                .map(|s| s.into())
                .collect::<Vec<_>>();

            Ok(VerbAttrs {
                definer: None,
                owner: Some(*owner),
                names: Some(name_strings),
                flags: Some(perms),
                args_spec: None,
                binary_type: None,
                binary: None,
            })
        }
        _ => Err(anyhow::anyhow!("Invalid verb info")),
    }
}

// set_verb_info (obj <object>, str <verb-desc>, list <info>) => none
async fn bf_set_verb_info<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 3 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };
    let Variant::List(info) = bf_args.args[2].variant() else {
        return Ok(Error(E_TYPE));
    };
    if info.len() != 3 {
        return Ok(Error(E_INVARG));
    }
    let Ok(update_attrs) = parse_verb_info(info) else {
        return Ok(Error(E_INVARG));
    };

    match bf_args.args[1].variant() {
        Variant::Str(verb_name) => {
            bf_args
                .world_state
                .update_verb(
                    bf_args.task_perms_who(),
                    *obj,
                    verb_name.as_str(),
                    update_attrs,
                )
                .await?;
        }
        Variant::Int(verb_index) => {
            let verb_index = *verb_index;
            if verb_index < 1 {
                return Ok(Error(E_INVARG));
            }
            let verb_index = (verb_index as usize) - 1;
            bf_args
                .world_state
                .update_verb_at_index(bf_args.task_perms_who(), *obj, verb_index, update_attrs)
                .await?;
        }
        _ => return Ok(Error(E_TYPE)),
    }

    Ok(Ret(v_none()))
}
bf_declare!(set_verb_info, bf_set_verb_info);

async fn bf_verb_args<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };
    let verbdef = match get_verbdef(*obj, bf_args.args[1].clone(), bf_args).await {
        Ok(v) => v,
        Err(e) => return Ok(Error(e)),
    };
    let args = verbdef.args();

    // Output is {dobj, prep, iobj} as strings
    let result = v_list(vec![
        v_str(args.dobj.to_string()),
        v_str(preposition_to_string(&args.prep)),
        v_str(args.iobj.to_string()),
    ]);
    Ok(Ret(result))
}
bf_declare!(verb_args, bf_verb_args);

fn parse_verb_args(verbinfo: &List) -> Result<VerbArgsSpec, anyhow::Error> {
    if verbinfo.len() != 3 {
        return Err(anyhow::anyhow!("Invalid verb args"));
    }
    match (
        verbinfo[0].variant(),
        verbinfo[1].variant(),
        verbinfo[2].variant(),
    ) {
        (Variant::Str(dobj_str), Variant::Str(prep_str), Variant::Str(iobj_str)) => {
            let (Some(dobj), Some(prep), Some(iobj)) = (
                ArgSpec::from_string(dobj_str.as_str()),
                parse_preposition_string(prep_str.as_str()),
                ArgSpec::from_string(iobj_str.as_str()),
            ) else {
                return Err(anyhow::anyhow!("Invalid verb args"));
            };
            Ok(VerbArgsSpec { dobj, prep, iobj })
        }
        _ => Err(anyhow::anyhow!("Invalid verb args")),
    }
}

// set_verb_args (obj <object>, str <verb-desc>, list <args>) => none
async fn bf_set_verb_args<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 3 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };
    let Variant::List(verbinfo) = bf_args.args[2].variant() else {
        return Ok(Error(E_TYPE));
    };
    if verbinfo.len() != 3 {
        return Ok(Error(E_INVARG));
    }
    let Ok(args) = parse_verb_args(verbinfo) else {
        return Ok(Error(E_INVARG));
    };
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
                    *obj,
                    verb_name.as_str(),
                    update_attrs,
                )
                .await?;
        }
        Variant::Int(verb_index) => {
            let verb_index = *verb_index;
            if verb_index < 1 {
                return Ok(Error(E_INVARG));
            }
            let verb_index = (verb_index as usize) - 1;
            bf_args
                .world_state
                .update_verb_at_index(bf_args.task_perms_who(), *obj, verb_index, update_attrs)
                .await?;
        }
        _ => return Ok(Error(E_TYPE)),
    }
    Ok(Ret(v_none()))
}
bf_declare!(set_verb_args, bf_set_verb_args);

async fn bf_verb_code(bf_args: &mut BfCallState<'_>) -> Result<BfRet, anyhow::Error> {
    //verb_code (obj object, str verb-desc [, fully-paren [, indent]]) => list
    if bf_args.args.len() < 2 || bf_args.args.len() > 4 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };
    // Verify caller is a programmer.
    if !bf_args
        .task_perms()
        .await?
        .flags
        .contains(ObjFlag::Programmer)
    {
        return Ok(Error(E_PERM));
    }
    let verbdef = match get_verbdef(*obj, bf_args.args[1].clone(), bf_args).await {
        Ok(v) => v,
        Err(e) => return Ok(Error(e)),
    };

    // If the verb is not binary type MOO, we don't support decompilation or listing
    // of it yet.
    if verbdef.binary_type() != BinaryType::LambdaMoo18X {
        warn!(object=?bf_args.args[0], verb=?bf_args.args[1], binary_type=?verbdef.binary_type(), 
            "verb_code: verb is not binary type MOO");
        return Ok(Error(E_TYPE));
    }

    // TODO: fully-paren and indent options. For now we ignore these.

    // Retrieve the binary for the verb.
    let verb_info = bf_args
        .world_state
        .retrieve_verb(bf_args.task_perms_who(), *obj, verbdef.uuid())
        .await?;

    // If the binary is empty, just return empty rather than try to decode it.
    if verb_info.binary().is_empty() {
        return Ok(Ret(v_empty_list()));
    }

    // Decode.
    let program = Program::from_sliceref(verb_info.binary());
    let decompiled = match program_to_tree(&program) {
        Ok(decompiled) => decompiled,
        Err(e) => {
            warn!(object=?bf_args.args[0], verb=?bf_args.args[1], error = ?e,
            binary_type=?verbdef.binary_type(), "verb_code: verb program could not be decompiled");
            return Ok(Error(E_INVARG));
        }
    };

    let unparsed = match unparse(&decompiled) {
        Ok(unparsed) => unparsed,
        Err(e) => {
            warn!(object=?bf_args.args[0], verb=?bf_args.args[1], error = ?e, 
                binary_type=?verbdef.binary_type(), 
            "verb_code: verb program could not be unparsed");
            return Ok(Error(E_INVARG));
        }
    };
    let split = unparsed.split('\n');
    let lines = split.map(v_str).collect::<Vec<_>>();
    Ok(Ret(v_list(lines)))
}
bf_declare!(verb_code, bf_verb_code);

// Function: list set_verb_code (obj object, str verb-desc, list code)
async fn bf_set_verb_code(bf_args: &mut BfCallState<'_>) -> Result<BfRet, anyhow::Error> {
    //set_verb_code (obj object, str verb-desc, list code) => none
    if bf_args.args.len() != 3 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };
    // Verify caller is a programmer.
    if !bf_args
        .task_perms()
        .await?
        .flags
        .contains(ObjFlag::Programmer)
    {
        return Ok(Error(E_PERM));
    }

    let verbdef = match get_verbdef(*obj, bf_args.args[1].clone(), bf_args).await {
        Ok(v) => v,
        Err(e) => return Ok(Error(e)),
    };

    // Right now set_verb_code is going to always compile to LambdaMOO 1.8.x. binary type.
    let binary_type = BinaryType::LambdaMoo18X;
    let program_code = match bf_args.args[2].variant() {
        Variant::List(code) => code,
        _ => return Ok(Error(E_TYPE)),
    };
    // Code should be a list of strings.
    // Which we will join (with linefeeds) into one string.
    let mut code_string = String::new();
    for line in program_code.iter() {
        let line = match line.variant() {
            Variant::Str(line) => line,
            _ => return Ok(Error(E_TYPE)),
        };
        code_string.push_str(line.as_str());
        code_string.push('\n');
    }
    // Now try to compile...
    let program = match compile(&code_string.as_str()) {
        Ok(program) => program,
        Err(e) => {
            // For set_verb_code(), the result is a list of strings, the error messages generated by the
            // MOO-code compiler during processing of code. If the list is non-empty, then
            // set_verb_code() did not install code; the program associated with the verb in question
            // is unchanged.
            let mut error_list = Vec::new();
            error_list.push(v_str(e.to_string().as_str()));
            return Ok(Ret(v_list(error_list)));
        }
    };
    // Now we have a program, we need to encode it.
    let binary = program.with_byte_buffer(|d| Vec::from(d));
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
        .update_verb_with_id(bf_args.task_perms_who(), *obj, verbdef.uuid(), update_attrs)
        .await?;
    Ok(Ret(v_none()))
}
bf_declare!(set_verb_code, bf_set_verb_code);

// Function: none add_verb (obj object, list info, list args)
async fn bf_add_verb(bf_args: &mut BfCallState<'_>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 3 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };
    let Variant::List(info) = bf_args.args[1].variant() else {
        return Ok(Error(E_TYPE));
    };
    let Variant::List(args) = bf_args.args[2].variant() else {
        return Ok(Error(E_TYPE));
    };

    // Verify caller is a programmer.
    if !bf_args
        .task_perms()
        .await?
        .flags
        .contains(ObjFlag::Programmer)
    {
        return Ok(Error(E_PERM));
    }

    let Ok(verbargs) = parse_verb_args(&args) else {
        return Ok(Error(E_INVARG));
    };

    let Ok(verbinfo) = parse_verb_info(&info) else {
        return Ok(Error(E_INVARG));
    };

    bf_args
        .world_state
        .add_verb(
            bf_args.task_perms_who(),
            *obj,
            verbinfo.names.unwrap(),
            verbinfo.owner.unwrap(),
            verbinfo.flags.unwrap(),
            verbargs,
            Vec::new(),
            BinaryType::LambdaMoo18X,
        )
        .await?;

    Ok(Ret(v_none()))
}
bf_declare!(add_verb, bf_add_verb);

//Function: none delete_verb (obj object, str verb-desc)
async fn bf_delete_verb(bf_args: &mut BfCallState<'_>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };

    // Verify caller is a programmer.
    if !bf_args
        .task_perms()
        .await?
        .flags
        .contains(ObjFlag::Programmer)
    {
        return Ok(Error(E_PERM));
    }

    let verbdef = match get_verbdef(*obj, bf_args.args[1].clone(), bf_args).await {
        Ok(v) => v,
        Err(e) => return Ok(Error(e)),
    };

    bf_args
        .world_state
        .remove_verb(bf_args.task_perms_who(), *obj, verbdef.uuid())
        .await?;

    Ok(Ret(v_none()))
}
bf_declare!(delete_verb, bf_delete_verb);

// Syntax:  disassemble (obj <object>, str <verb-desc>)   => list
//
// Returns a (longish) list of strings giving a listing of the server's internal ``compiled'' form of the verb as specified by <verb-desc>
// on <object>.  This format is not documented and may indeed change from release to release, but some programmers may nonetheless find
// the output of `disassemble()' interesting to peruse as a way to gain a deeper appreciation of how the server works.
async fn bf_disassemble(bf_args: &mut BfCallState<'_>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };

    let verbdef = match get_verbdef(*obj, bf_args.args[1].clone(), bf_args).await {
        Ok(v) => v,
        Err(e) => return Ok(Error(e)),
    };

    if verbdef.binary_type() != BinaryType::LambdaMoo18X {
        warn!(object=?bf_args.args[0], verb=?bf_args.args[1], binary_type=?verbdef.binary_type(),
            "disassemble: verb is not binary type MOO");
        return Ok(Error(E_TYPE));
    }

    let verb_info = bf_args
        .world_state
        .retrieve_verb(bf_args.task_perms_who(), *obj, verbdef.uuid())
        .await?;

    if verb_info.binary().is_empty() {
        return Ok(Ret(v_empty_list()));
    }

    let program = Program::from_sliceref(verb_info.binary());

    // The output of disassemble is a list of strings, one for each instruction in the verb's program.
    // But we also want some basic information on # of labels. Fork vectors. Jazz like that.
    // But I'll just keep it simple for now.

    let mut disassembly = Vec::new();
    // Write literals indexed by their offset #
    disassembly.push(v_str("LITERALS:"));
    for (i, l) in program.literals.iter().enumerate() {
        disassembly.push(v_string(format!("{: >3}: {}", i, l.to_literal())));
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
    for (i, v) in program.var_names.names.iter().enumerate() {
        disassembly.push(v_string(format!("{: >3}: {}", i, v)));
    }

    // TODO: print fork vectors

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

    Ok(Ret(v_list(disassembly)))
}
bf_declare!(disassemble, bf_disassemble);

impl VM {
    pub(crate) fn register_bf_verbs(&mut self) -> Result<(), anyhow::Error> {
        self.builtins[offset_for_builtin("verb_info")] = Arc::new(Box::new(BfVerbInfo {}));
        self.builtins[offset_for_builtin("set_verb_info")] = Arc::new(Box::new(BfSetVerbInfo {}));
        self.builtins[offset_for_builtin("verb_args")] = Arc::new(Box::new(BfVerbArgs {}));
        self.builtins[offset_for_builtin("set_verb_args")] = Arc::new(Box::new(BfSetVerbArgs {}));
        self.builtins[offset_for_builtin("verb_code")] = Arc::new(Box::new(BfVerbCode {}));
        self.builtins[offset_for_builtin("set_verb_code")] = Arc::new(Box::new(BfSetVerbCode {}));
        self.builtins[offset_for_builtin("add_verb")] = Arc::new(Box::new(BfAddVerb {}));
        self.builtins[offset_for_builtin("delete_verb")] = Arc::new(Box::new(BfDeleteVerb {}));
        self.builtins[offset_for_builtin("disassemble")] = Arc::new(Box::new(BfDisassemble {}));
        Ok(())
    }
}
