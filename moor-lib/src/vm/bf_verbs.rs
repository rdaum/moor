use std::sync::Arc;

use async_trait::async_trait;
use moor_value::model::defset::HasUuid;
use moor_value::model::objects::ObjFlag;
use moor_value::AsByteBuffer;
use tracing::warn;

use moor_value::model::r#match::{ArgSpec, VerbArgsSpec};
use moor_value::model::verbs::{BinaryType, VerbAttrs, VerbFlag};
use moor_value::util::bitenum::BitEnum;
use moor_value::var::error::Error::{E_INVARG, E_PERM, E_TYPE};
use moor_value::var::variant::Variant;
use moor_value::var::{v_list, v_none, v_objid, v_str, v_string};

use crate::bf_declare;
use crate::compiler::builtins::offset_for_builtin;
use crate::compiler::decompile::program_to_tree;
use crate::compiler::unparse::unparse;
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
    match (info[0].variant(), info[1].variant(), info[2].variant()) {
        (Variant::Obj(owner), Variant::Str(perms_str), Variant::Str(names)) => {
            let mut perms = BitEnum::new();
            for c in perms_str.as_str().chars() {
                match c {
                    'r' => perms |= VerbFlag::Read,
                    'w' => perms |= VerbFlag::Write,
                    'x' => perms |= VerbFlag::Exec,
                    'd' => perms |= VerbFlag::Debug,
                    _ => return Ok(Error(E_INVARG)),
                }
            }

            // Split the names string into a list of strings.
            let name_strings = names
                .as_str()
                .split(' ')
                .map(|s| s.into())
                .collect::<Vec<_>>();

            let update_attrs = VerbAttrs {
                definer: None,
                owner: Some(*owner),
                names: Some(name_strings),
                flags: Some(perms),
                args_spec: None,
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
                        .update_verb_at_index(
                            bf_args.task_perms_who(),
                            *obj,
                            verb_index,
                            update_attrs,
                        )
                        .await?;
                }
                _ => return Ok(Error(E_TYPE)),
            }

            Ok(Ret(v_none()))
        }
        _ => Ok(Error(E_INVARG)),
    }
}
bf_declare!(set_verb_info, bf_set_verb_info);

async fn bf_verb_args<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };
    let args = match bf_args.args[1].variant() {
        Variant::Str(verb_desc) => {
            let verb_desc = verb_desc.as_str();
            let verb_info = bf_args
                .world_state
                .get_verb(bf_args.task_perms_who(), *obj, verb_desc)
                .await?;
            verb_info.args()
        }
        Variant::Int(verb_index) => {
            let verb_index = *verb_index;
            if verb_index < 1 {
                return Ok(Error(E_INVARG));
            }
            let verb_index = (verb_index as usize) - 1;
            let verb_info = bf_args
                .world_state
                .get_verb_at_index(bf_args.task_perms_who(), *obj, verb_index)
                .await?;
            verb_info.args()
        }
        _ => return Ok(Error(E_TYPE)),
    };
    // Output is {dobj, prep, iobj} as strings
    let result = v_list(vec![
        v_str(args.dobj.to_string()),
        v_str(preposition_to_string(&args.prep)),
        v_str(args.iobj.to_string()),
    ]);
    Ok(Ret(result))
}
bf_declare!(verb_args, bf_verb_args);

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
                return Ok(Error(E_INVARG));
            };
            let args = VerbArgsSpec { dobj, prep, iobj };
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
                        .update_verb_at_index(
                            bf_args.task_perms_who(),
                            *obj,
                            verb_index,
                            update_attrs,
                        )
                        .await?;
                }
                _ => return Ok(Error(E_TYPE)),
            }
            Ok(Ret(v_none()))
        }
        _ => Ok(Error(E_INVARG)),
    }
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

    let verbdef = match bf_args.args[1].variant() {
        Variant::Str(verb_desc) => {
            let verb_desc = verb_desc.as_str();
            bf_args
                .world_state
                .get_verb(bf_args.task_perms_who(), *obj, verb_desc)
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
        _ => return Ok(Error(E_TYPE)),
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

impl VM {
    pub(crate) fn register_bf_verbs(&mut self) -> Result<(), anyhow::Error> {
        self.builtins[offset_for_builtin("verb_info")] = Arc::new(Box::new(BfVerbInfo {}));
        self.builtins[offset_for_builtin("set_verb_info")] = Arc::new(Box::new(BfSetVerbInfo {}));
        self.builtins[offset_for_builtin("verb_args")] = Arc::new(Box::new(BfVerbArgs {}));
        self.builtins[offset_for_builtin("set_verb_args")] = Arc::new(Box::new(BfSetVerbArgs {}));
        self.builtins[offset_for_builtin("verb_code")] = Arc::new(Box::new(BfVerbCode {}));
        Ok(())
    }
}
