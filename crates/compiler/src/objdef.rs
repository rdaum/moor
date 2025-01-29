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

use crate::codegen::compile_tree;
use crate::parse::moo::{MooParser, Rule};
use crate::parse::unquote_str;
use crate::CompileOptions;
use bytes::Bytes;
use moor_values::model::{
    ArgSpec, CompileError, ObjFlag, PrepSpec, PropFlag, PropPerms, VerbArgsSpec, VerbFlag,
};
use moor_values::util::BitEnum;
use moor_values::Error::{
    E_ARGS, E_DIV, E_FLOAT, E_INVARG, E_INVIND, E_MAXREC, E_NACC, E_NONE, E_PERM, E_PROPNF,
    E_QUOTA, E_RANGE, E_RECMOVE, E_TYPE, E_VARNF, E_VERBNF,
};
use moor_values::{
    v_err, v_float, v_int, v_list, v_map, v_obj, v_str, AsByteBuffer, Obj, Symbol, Var, NOTHING,
};
use pest::error::LineColLocation;
use pest::iterators::{Pair, Pairs};
use pest::Parser;
use std::str::FromStr;

pub struct ObjectDefinition {
    pub oid: Obj,
    pub name: String,
    pub parent: Obj,
    pub owner: Obj,
    pub location: Obj,
    pub flags: BitEnum<ObjFlag>,

    pub verbs: Vec<ObjVerbDef>,
    pub propsdefs: Vec<ObjPropDef>,
    pub propovrrds: Vec<ObjPropSet>,
}

pub struct ObjVerbDef {
    pub names: Vec<Symbol>,
    pub argspec: VerbArgsSpec,
    pub owner: Obj,
    pub flags: BitEnum<VerbFlag>,
    pub binary: Bytes,
}

pub struct ObjPropDef {
    pub name: Symbol,
    pub perms: PropPerms,
    pub value: Option<Var>,
}

pub struct ObjPropSet {
    pub name: Symbol,
    pub perms_update: Option<PropPerms>,
    pub value: Option<Var>,
}

#[derive(thiserror::Error, Debug)]
pub enum ObjDefParseError {
    #[error("Failed to compile verb: {0}")]
    VerbCompileError(CompileError),
    #[error("Failed to parse verb flags: {0}")]
    BadVerbFlags(String),
    #[error("Failed to parse verb argspec: {0}")]
    BadVerbArgspec(String),
    #[error("Failed to parse propflags: {0}")]
    BadPropFlags(String),
}

fn parse_boolean_literal(pair: pest::iterators::Pair<Rule>) -> Result<bool, ObjDefParseError> {
    let str = pair.as_str();
    match str.to_lowercase().as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => {
            panic!("Expected boolean literal, got {:?}", pair);
        }
    }
}
fn parse_literal_list(pairs: pest::iterators::Pairs<Rule>) -> Result<Var, ObjDefParseError> {
    let mut list = vec![];
    for pair in pairs {
        let l = parse_literal(pair)?;
        list.push(l);
    }
    Ok(v_list(&list))
}

fn parse_literal_map(pairs: pest::iterators::Pairs<Rule>) -> Result<Var, ObjDefParseError> {
    let mut elements = vec![];
    for r in pairs {
        elements.push(parse_literal(r)?);
    }
    let pairs: Vec<_> = elements
        .chunks(2)
        .map(|pair| {
            let key = pair[0].clone();
            let value = pair[1].clone();
            (key, value)
        })
        .collect();
    Ok(v_map(&pairs))
}

fn parse_literal(pair: pest::iterators::Pair<Rule>) -> Result<Var, ObjDefParseError> {
    match pair.as_rule() {
        Rule::atom => {
            let pair = pair.into_inner().next().unwrap();
            parse_literal_atom(pair)
        }
        Rule::literal_list => {
            let pairs = pair.into_inner();
            parse_literal_list(pairs)
        }
        Rule::literal_map => {
            let pairs = pair.into_inner();

            parse_literal_map(pairs)
        }
        _ => {
            panic!("Unimplemented literal: {:?}", pair);
        }
    }
}

fn parse_object_literal(pair: pest::iterators::Pair<Rule>) -> Result<Obj, ObjDefParseError> {
    match pair.as_rule() {
        Rule::object => {
            let ostr = &pair.as_str()[1..];
            let oid = i32::from_str(ostr).unwrap();
            let objid = Obj::mk_id(oid);
            Ok(objid)
        }
        _ => {
            panic!("Unexpected object literal: {:?}", pair);
        }
    }
}

fn parse_string_literal(pair: pest::iterators::Pair<Rule>) -> Result<String, ObjDefParseError> {
    let string = pair.as_str();
    let parsed = unquote_str(string).map_err(ObjDefParseError::VerbCompileError)?;
    Ok(parsed)
}

fn parse_literal_atom(pair: pest::iterators::Pair<Rule>) -> Result<Var, ObjDefParseError> {
    match pair.as_rule() {
        Rule::object => {
            let objid = parse_object_literal(pair)?;
            Ok(v_obj(objid))
        }
        Rule::integer => Ok(v_int(
            pair.as_str()
                .parse::<i64>()
                .map_err(|e| {
                    CompileError::StringLexError(format!(
                        "Failed to parse '{}' to i64: {e}",
                        pair.as_str()
                    ))
                })
                .map_err(ObjDefParseError::VerbCompileError)?,
        )),
        Rule::float => Ok(v_float(
            pair.as_str()
                .parse::<f64>()
                .map_err(|e| {
                    CompileError::StringLexError(format!(
                        "Failed to parse '{}' to f64: {e}",
                        pair.as_str()
                    ))
                })
                .map_err(ObjDefParseError::VerbCompileError)?,
        )),
        Rule::string => {
            let str = parse_string_literal(pair)?;
            Ok(v_str(&str))
        }
        Rule::err => {
            let e = pair.as_str();
            Ok(match e.to_lowercase().as_str() {
                "e_args" => v_err(E_ARGS),
                "e_div" => v_err(E_DIV),
                "e_float" => v_err(E_FLOAT),
                "e_invarg" => v_err(E_INVARG),
                "e_invind" => v_err(E_INVIND),
                "e_maxrec" => v_err(E_MAXREC),
                "e_nacc" => v_err(E_NACC),
                "e_none" => v_err(E_NONE),
                "e_perm" => v_err(E_PERM),
                "e_propnf" => v_err(E_PROPNF),
                "e_quota" => v_err(E_QUOTA),
                "e_range" => v_err(E_RANGE),
                "e_recmove" => v_err(E_RECMOVE),
                "e_type" => v_err(E_TYPE),
                "e_varnf" => v_err(E_VARNF),
                "e_verbnf" => v_err(E_VERBNF),
                &_ => {
                    panic!("unknown error")
                }
            })
        }
        _ => {
            panic!("Unimplemented atom: {:?}", pair);
        }
    }
}

pub fn compile_object_definitions(
    objdef: &str,
    options: &CompileOptions,
) -> Result<Vec<ObjectDefinition>, ObjDefParseError> {
    let mut pairs = match MooParser::parse(Rule::objects_file, objdef) {
        Ok(pairs) => pairs,
        Err(e) => {
            let ((line, column), end_line_col) = match e.line_col {
                LineColLocation::Pos(lc) => (lc, None),
                LineColLocation::Span(begin, end) => (begin, Some(end)),
            };

            return Err(ObjDefParseError::VerbCompileError(
                CompileError::ParseError {
                    line,
                    column,
                    end_line_col,
                    context: e.line().to_string(),
                    message: e.variant.message().to_string(),
                },
            ));
        }
    };

    let ofile = pairs.next().unwrap();
    let pairs = match ofile.as_rule() {
        Rule::objects_file => ofile.into_inner(),
        _ => {
            panic!("Expected object file, got {:?}", ofile);
        }
    };

    let mut objdefs = vec![];
    for pair in pairs {
        match pair.as_rule() {
            Rule::object_definition => {
                objdefs.push(compile_object_definition(pair, options)?);
            }
            Rule::EOI => {
                break;
            }
            _ => {
                panic!("Unexpected rule: {:?}", pair);
            }
        }
    }

    Ok(objdefs)
}

fn compile_object_definition(
    pair: Pair<Rule>,
    options: &CompileOptions,
) -> Result<ObjectDefinition, ObjDefParseError> {
    // Now walk the tree of object / verb / property definitions and extract out the relevant info,
    // also attempting to compile each verb.
    let mut objdef = ObjectDefinition {
        oid: NOTHING,
        name: "".to_string(),
        parent: NOTHING,
        owner: NOTHING,
        location: NOTHING,
        flags: Default::default(),
        verbs: Default::default(),
        propsdefs: Default::default(),
        propovrrds: Default::default(),
    };

    let mut pairs = pair.into_inner();
    let let_oid = pairs.next().unwrap();
    match let_oid.as_rule() {
        Rule::object => {
            let oid = parse_object_literal(let_oid)?;
            objdef.oid = oid;
        }
        _ => {
            panic!("Expected object, got {:?}", let_oid);
        }
    }

    // Next is object attributes
    let object_attrs = pairs.next().unwrap();
    match object_attrs.as_rule() {
        Rule::object_attributes => {
            let attrs = object_attrs.into_inner();
            for attr in attrs {
                let attr_pair = attr.into_inner().next().unwrap();
                let rule = attr_pair.as_rule();
                match rule {
                    Rule::parent_attr => {
                        let inner = attr_pair.into_inner().next().unwrap();
                        objdef.parent = parse_object_literal(inner)?;
                    }
                    Rule::name_attr => {
                        let inner = attr_pair.into_inner().next().unwrap();
                        objdef.name = parse_string_literal(inner)?;
                    }
                    Rule::owner_attr => {
                        let inner = attr_pair.into_inner().next().unwrap();
                        objdef.owner = parse_object_literal(inner)?;
                    }
                    Rule::location_attr => {
                        let inner = attr_pair.into_inner().next().unwrap();
                        objdef.location = parse_object_literal(inner)?;
                    }
                    Rule::wizard_attr => {
                        let inner = attr_pair.into_inner().next().unwrap();
                        let is_wizard = parse_boolean_literal(inner)?;
                        if is_wizard {
                            objdef.flags.set(ObjFlag::Wizard);
                        }
                    }
                    Rule::prog_attr => {
                        let inner = attr_pair.into_inner().next().unwrap();
                        let is_prog = parse_boolean_literal(inner)?;
                        if is_prog {
                            objdef.flags.set(ObjFlag::Programmer);
                        }
                    }
                    Rule::player_attr => {
                        let inner = attr_pair.into_inner().next().unwrap();
                        let is_user = parse_boolean_literal(inner)?;
                        if is_user {
                            objdef.flags.set(ObjFlag::User);
                        }
                    }
                    Rule::fertile_attr => {
                        let inner = attr_pair.into_inner().next().unwrap();
                        let is_fertile = parse_boolean_literal(inner)?;
                        if is_fertile {
                            objdef.flags.set(ObjFlag::Fertile);
                        }
                    }
                    Rule::read_attr => {
                        let inner = attr_pair.into_inner().next().unwrap();
                        let is_readable = parse_boolean_literal(inner)?;
                        if is_readable {
                            objdef.flags.set(ObjFlag::Read);
                        }
                    }
                    Rule::write_attr => {
                        let inner = attr_pair.into_inner().next().unwrap();
                        let is_readable = parse_boolean_literal(inner)?;
                        if is_readable {
                            objdef.flags.set(ObjFlag::Write);
                        }
                    }
                    _ => {
                        panic!("Unexpected object attribute: {:?}", attr_pair);
                    }
                }
            }
        }
        _ => {
            panic!("Expected object attributes, got {:?}", object_attrs);
        }
    }

    // Now it's either properties or verbs until end of program
    for pair in pairs {
        match pair.as_rule() {
            Rule::verb_decl => {
                let inner = pair.into_inner();
                let vd = parse_verb_decl(inner, options)?;
                objdef.verbs.push(vd);
            }
            Rule::prop_def => {
                let inner = pair.into_inner();
                let pd = parse_prop_def(inner)?;
                objdef.propsdefs.push(pd);
            }
            Rule::prop_set => {
                let inner = pair.into_inner();
                let ps = parse_prop_set(inner)?;
                objdef.propovrrds.push(ps);
            }
            Rule::EOI => {
                return Ok(objdef);
            }
            _ => {
                panic!("Unexpected rule: {:?}", pair);
            }
        }
    }
    Ok(objdef)
}

fn parse_prop_set(mut pairs: Pairs<Rule>) -> Result<ObjPropSet, ObjDefParseError> {
    let name = Symbol::mk(pairs.next().unwrap().as_str().trim());

    // Next is either propinfo -> perms override, or if not, it's straight to the value.
    // So check what it is first
    let mut next = pairs.next();
    let pr = next.clone().unwrap();
    let perms_update = if pr.as_rule() == Rule::propinfo {
        // owner, flags
        let mut inner = pr.into_inner();
        let attr_pair = inner.next().unwrap();
        let attr_inner = attr_pair.into_inner().next().unwrap();
        let owner = parse_object_literal(attr_inner)?;

        let flags_pair = inner.next().unwrap();
        let flags_inner = flags_pair.into_inner().next().unwrap();
        let flags_str = flags_inner.as_str();
        // TODO : proper error
        let flags = PropFlag::parse_str(flags_str).expect("Failed to parse flags");
        next = pairs.next();
        Some(PropPerms::new(owner, flags))
    } else {
        None
    };

    let value = match &next {
        Some(n) => match n.as_rule() {
            Rule::literal => {
                let inner = n.clone().into_inner().next().unwrap();
                Some(parse_literal(inner)?)
            }
            _ => {
                panic!("Expected literal, got {:?}", next);
            }
        },
        None => None,
    };

    Ok(ObjPropSet {
        name,
        perms_update,
        value,
    })
}

fn parse_prop_def(mut pairs: Pairs<Rule>) -> Result<ObjPropDef, ObjDefParseError> {
    let name = Symbol::mk(pairs.next().unwrap().as_str().trim());

    let propinfo = pairs.next().unwrap();
    let perms = match propinfo.as_rule() {
        Rule::propinfo => {
            // owner, flags
            let mut inner = propinfo.into_inner();
            let attr_pair = inner.next().unwrap();
            let attr_inner = attr_pair.into_inner().next().unwrap();
            let owner = parse_object_literal(attr_inner)?;

            let flags_pair = inner.next().unwrap();
            let flags_inner = flags_pair.into_inner().next().unwrap();
            let flags_str = flags_inner.as_str();
            // TODO : proper error
            let flags = PropFlag::parse_str(flags_str).expect("Failed to parse flags");

            PropPerms::new(owner, flags)
        }
        _ => {
            panic!("Expected propflags, got {:?}", propinfo);
        }
    };

    let value = match pairs.next() {
        None => None,
        Some(pair) => match pair.as_rule() {
            Rule::literal => {
                let inner = pair.into_inner().next().unwrap();
                Some(parse_literal(inner)?)
            }
            _ => {
                panic!("Expected literal, got {:?}", pair);
            }
        },
    };

    Ok(ObjPropDef { name, perms, value })
}

fn parse_verb_decl(
    mut pairs: Pairs<Rule>,
    compile_options: &CompileOptions,
) -> Result<ObjVerbDef, ObjDefParseError> {
    // First is the verb_names
    let mut vd = ObjVerbDef {
        names: vec![],
        argspec: VerbArgsSpec::this_none_this(),
        owner: NOTHING,
        flags: VerbFlag::r(),
        binary: Default::default(),
    };

    let verb_names = pairs.next().unwrap();
    match verb_names.as_rule() {
        Rule::verb_names => {
            let inner = verb_names.into_inner();
            for name in inner {
                let name = Symbol::mk(name.as_str().trim());
                vd.names.push(name);
            }
        }
        _ => {
            panic!("Expected verb names, got {:?}", verb_names);
        }
    }

    // Then the verbargpsec
    let argspec_pair = pairs.next().unwrap();
    match argspec_pair.as_rule() {
        Rule::verbargspec => {
            let mut inner = argspec_pair.into_inner();
            // 3 parts: dobj, prep, iobj
            let dobj = inner.next().unwrap().as_str();
            // TODO: need an error for this, and to move from CompilerError to our own error set
            let dobj = ArgSpec::from_string(dobj).expect("Failed to parse dobj");
            let prep = inner.next().unwrap().as_str();
            let prep = PrepSpec::parse(prep).expect("Failed to parse prep");
            let iobj = inner.next().unwrap().as_str();
            let iobj = ArgSpec::from_string(iobj).expect("Failed to parse iobj");

            vd.argspec = VerbArgsSpec { dobj, prep, iobj }
        }
        _ => {
            panic!("Expected verb argspec, got {:?}", argspec_pair);
        }
    }

    // Now owner & flags
    let owner_pair = pairs.next().unwrap();
    match owner_pair.as_rule() {
        Rule::owner_attr => {
            let inner = owner_pair.into_inner().next().unwrap();
            vd.owner = parse_object_literal(inner)?;
        }
        _ => {
            panic!("Expected owner, got {:?}", owner_pair);
        }
    }

    // Then flags, which is a string like 'rxd' etc.
    let flags_pair = pairs.next().unwrap();
    match flags_pair.as_rule() {
        Rule::flags_attr => {
            let inner = flags_pair.into_inner().next().unwrap();
            let content = inner.as_str();
            vd.flags = VerbFlag::parse_str(content).expect("Failed to parse flags");
        }
        _ => {
            panic!("Expected flags, got {:?}", flags_pair);
        }
    }

    // Now the verb body, which we will attempt to compile with a specialized form of the regular
    // compiler.
    let verb_body = pairs.next().unwrap();
    let program = match verb_body.as_rule() {
        Rule::verb_statements => {
            let inner = verb_body.into_inner();
            compile_tree(inner, compile_options.clone())
                .map_err(ObjDefParseError::VerbCompileError)?
        }
        _ => {
            panic!("Expected verb body, got {:?}", verb_body);
        }
    };

    // Encode the program
    vd.binary = program
        .with_byte_buffer(|d| Vec::from(d))
        .expect("Failed to encode program byte stream")
        .into();

    Ok(vd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_values::model::Preposition;
    use moor_values::Variant;

    /// Just a simple objdef no verbs or props
    #[test]
    fn simple_object_def() {
        let spec = r#"
        object #1
            parent: #1
            name: "Test Object"
            location: #3
            wizard: false
            programmer: false
            player: false
            fertile: true
            readable: true
        endobject
        "#;

        let odef = &compile_object_definitions(spec, &CompileOptions::default()).unwrap()[0];
        assert_eq!(odef.oid, Obj::mk_id(1));
        assert_eq!(odef.name, "Test Object");
        assert_eq!(odef.parent, Obj::mk_id(1));
        assert_eq!(odef.location, Obj::mk_id(3));
        assert!(!odef.flags.contains(ObjFlag::Wizard));
        assert!(!odef.flags.contains(ObjFlag::Programmer));
        assert!(!odef.flags.contains(ObjFlag::User));
        assert!(odef.flags.contains(ObjFlag::Fertile));
        assert!(odef.flags.contains(ObjFlag::Read));
        assert!(!odef.flags.contains(ObjFlag::Write));
    }

    // Verify that verb definitions are working
    #[test]
    fn object_with_verbdefs() {
        let spec = r#"
                object #1
                    parent: #1
                    name: "Test Object"
                    location: #3
                    wizard: false
                    programmer: false
                    player: false
                    fertile: true
                    readable: true

                    verb look_self, look_* (this to any) owner: #2 flags: rxd
                        return 5;
                    endverb

                    verb another_test (this none this) owner: #2 flags: r
                        player:tell("here is something");
                    endverb
                endobject"#;

        let odef = &compile_object_definitions(spec, &CompileOptions::default()).unwrap()[0];
        assert_eq!(odef.verbs.len(), 2);

        assert_eq!(odef.verbs[0].names.len(), 2);
        assert_eq!(odef.verbs[0].names[0], Symbol::mk("look_self"));
        assert_eq!(odef.verbs[0].names[1], Symbol::mk("look_*"));
        assert_eq!(odef.verbs[0].argspec.dobj, ArgSpec::This);
        assert_eq!(
            odef.verbs[0].argspec.prep,
            PrepSpec::Other(Preposition::AtTo)
        );
        assert_eq!(odef.verbs[0].argspec.iobj, ArgSpec::Any);
        assert_eq!(odef.verbs[0].owner, Obj::mk_id(2));
        assert_eq!(odef.verbs[0].flags, VerbFlag::rxd());
        assert!(!odef.verbs[0].binary.is_empty());

        assert_eq!(odef.verbs[1].names.len(), 1);
        assert_eq!(odef.verbs[1].names[0], Symbol::mk("another_test"));
        assert_eq!(odef.verbs[1].argspec.dobj, ArgSpec::This);
        assert_eq!(odef.verbs[1].argspec.prep, PrepSpec::None);
        assert_eq!(odef.verbs[1].argspec.iobj, ArgSpec::This);
        assert_eq!(odef.verbs[1].owner, Obj::mk_id(2));
        assert_eq!(odef.verbs[1].flags, VerbFlag::r());
        assert!(!odef.verbs[1].binary.is_empty());
    }

    // Verify property definition / setting
    #[test]
    fn object_with_property_defs() {
        let spec = r#"
                object #1
                    parent: #1
                    name: "Test Object"
                    location: #3
                    wizard: false
                    programmer: false
                    player: false
                    fertile: true
                    readable: true

                    // Declares a property named "description" with owner #2 and flags rc
                    property description (owner: #2, flags: rc) = "This is a test object";
                    property other (owner: #2, flags: rc);
                endobject"#;
        let odef = &compile_object_definitions(spec, &CompileOptions::default()).unwrap()[0];

        assert_eq!(odef.propsdefs.len(), 2);
        assert_eq!(odef.propsdefs[0].name, Symbol::mk("description"));
        assert_eq!(odef.propsdefs[0].name, Symbol::mk("description"));
        assert_eq!(odef.propsdefs[0].perms.owner(), Obj::mk_id(2));
        assert_eq!(odef.propsdefs[0].perms.flags(), PropFlag::rc());
        let Variant::Str(s) = odef.propsdefs[0].value.as_ref().unwrap().variant() else {
            panic!("Expected string value");
        };
        assert_eq!(s.as_string(), "This is a test object");

        assert_eq!(odef.propsdefs[1].name, Symbol::mk("other"));
        assert_eq!(odef.propsdefs[1].perms.owner(), Obj::mk_id(2));
        assert_eq!(odef.propsdefs[1].perms.flags(), PropFlag::rc());
        assert!(odef.propsdefs[1].value.is_none());
    }

    #[test]
    fn object_with_property_override() {
        let spec = r#"
                object #1
                    parent: #1
                    name: "Test Object"
                    location: #3
                    wizard: false
                    programmer: false
                    player: false
                    fertile: true
                    readable: true

                    // Overrides the description property from the parent
                    override description = "This is a test object";

                    override other (owner: #2, flags: rc) = "test";
                endobject"#;
        let odef = &compile_object_definitions(spec, &CompileOptions::default()).unwrap()[0];

        assert_eq!(odef.propovrrds.len(), 2);
        assert_eq!(odef.propovrrds[0].name, Symbol::mk("description"));
        let Variant::Str(s) = odef.propovrrds[0].value.as_ref().unwrap().variant() else {
            panic!("Expected string value");
        };
        assert_eq!(s.as_string(), "This is a test object");

        assert_eq!(odef.propovrrds[1].name, Symbol::mk("other"));
        let Variant::Str(s) = odef.propovrrds[1].value.as_ref().unwrap().variant() else {
            panic!("Expected string value");
        };
        assert_eq!(s.as_string(), "test");
    }

    #[test]
    fn a_mix_of_the_above_parses() {
        let spec = r#"
                object #1
                    // Testing a C++ Style comment
                    parent: #1
                    name: "Test Object"
                    location: #3
                    wizard: false
                    programmer: false
                    player: false
                    fertile: true
                    readable: true

                    /* C style comment
                    *
                    */
                    property description (owner: #2, flags: rc) = "This is a test object";
                    property other (owner: #2, flags: rc);
                    override description = "This is a test object";
                    override other (owner: #2, flags: rc) = "test";

                    verb look_self, look_* (this to any) owner: #2 flags: rxd
                        return 5;
                    endverb

                    verb another_test (this none this) owner: #2 flags: r
                        player:tell("here is something");
                    endverb
                endobject"#;
        compile_object_definitions(spec, &CompileOptions::default()).unwrap();
    }
}
