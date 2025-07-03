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

use crate::CompileOptions;
use crate::ObjDefParseError::VerbCompileError;
use crate::codegen::compile_tree;
use crate::parse::moo::{MooParser, Rule};
use crate::parse::unquote_str;
use itertools::Itertools;
use moor_common::model::{
    ArgSpec, CompileContext, CompileError, ObjFlag, PrepSpec, PropFlag, PropPerms, VerbArgsSpec,
    VerbFlag,
};
use moor_common::util::BitEnum;
use moor_var::program::ProgramType;
use moor_var::{
    ErrorCode, List, NOTHING, Obj, Symbol, Var, VarType, v_bool, v_err, v_float, v_flyweight,
    v_int, v_list, v_map, v_obj, v_str,
};
use pest::Parser;
use pest::error::LineColLocation;
use pest::iterators::{Pair, Pairs};
use std::collections::HashMap;
use std::str::FromStr;

pub struct ObjFileContext(HashMap<Symbol, Var>);
impl Default for ObjFileContext {
    fn default() -> Self {
        Self::new()
    }
}

impl ObjFileContext {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
}

pub struct ObjectDefinition {
    pub oid: Obj,
    pub name: String,
    pub parent: Obj,
    pub owner: Obj,
    pub location: Obj,
    pub flags: BitEnum<ObjFlag>,

    pub verbs: Vec<ObjVerbDef>,
    pub property_definitions: Vec<ObjPropDef>,
    pub property_overrides: Vec<ObjPropOverride>,
}

pub struct ObjVerbDef {
    pub names: Vec<Symbol>,
    pub argspec: VerbArgsSpec,
    pub owner: Obj,
    pub flags: BitEnum<VerbFlag>,
    pub program: ProgramType,
}

pub struct ObjPropDef {
    pub name: Symbol,
    pub perms: PropPerms,
    pub value: Option<Var>,
}

pub struct ObjPropOverride {
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
    #[error("Constant not found: {0}")]
    ConstantNotFound(String),
    #[error("Bad attribute type: {0:?}")]
    BadAttributeType(VarType),
}

fn parse_boolean_literal(pair: Pair<Rule>) -> Result<bool, ObjDefParseError> {
    let str = pair.as_str();
    match str.to_lowercase().as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => {
            panic!("Expected boolean literal, got {pair:?}");
        }
    }
}
fn parse_literal_list(
    context: &mut ObjFileContext,
    pairs: Pairs<Rule>,
) -> Result<Var, ObjDefParseError> {
    let mut list = vec![];
    for pair in pairs {
        let l = parse_literal(context, pair)?;
        list.push(l);
    }
    Ok(v_list(&list))
}

fn parse_literal_map(
    context: &mut ObjFileContext,
    pairs: Pairs<Rule>,
) -> Result<Var, ObjDefParseError> {
    let mut elements = vec![];
    for r in pairs {
        elements.push(parse_literal(context, r)?);
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

fn parse_literal(context: &mut ObjFileContext, pair: Pair<Rule>) -> Result<Var, ObjDefParseError> {
    match pair.as_rule() {
        Rule::literal_atom => {
            let pair = pair.into_inner().next().unwrap();
            parse_literal_atom(context, pair)
        }
        Rule::literal_list => {
            let pairs = pair.into_inner();
            parse_literal_list(context, pairs)
        }
        Rule::literal_map => {
            let pairs = pair.into_inner();

            parse_literal_map(context, pairs)
        }
        Rule::literal => {
            let literal = pair.into_inner().next().unwrap();
            parse_literal(context, literal)
        }
        Rule::literal_flyweight => {
            // Three components:
            // 1. The delegate object
            // 2. The slots
            // 3. The contents
            let mut parts = pair.into_inner();
            let delegate =
                parse_literal(context, parts.next().unwrap().into_inner().next().unwrap())?;
            let Some(delegate) = delegate.as_object() else {
                panic!("Expected object literal, got {delegate:?}");
            };
            let mut slots = vec![];
            let mut contents = vec![];

            // Parse the remaining parts: optional slots, optional contents
            for next in parts {
                match next.as_rule() {
                    Rule::literal_flyweight_slots => {
                        // Parse the slots, they're a sequence of ident, expr pairs.
                        // Collect them into two iterators,
                        let slot_pairs = next.clone().into_inner().chunks(2);
                        for mut pair in &slot_pairs {
                            let slot_name = Symbol::mk(pair.next().unwrap().as_str());

                            // "delegate" and "slots" are forbidden slot names.
                            if slot_name == Symbol::mk("delegate")
                                || slot_name == Symbol::mk("slots")
                            {
                                return Err(VerbCompileError(CompileError::BadSlotName(
                                    CompileContext::new(next.line_col()),
                                    slot_name.to_string(),
                                )));
                            }

                            let slot_expr = parse_literal(context, pair.next().unwrap())?;
                            slots.push((slot_name, slot_expr));
                        }
                    }
                    Rule::literal_flyweight_contents => {
                        let pairs = next.into_inner();
                        for pair in pairs {
                            let l = parse_literal(context, pair)?;
                            contents.push(l);
                        }
                    }
                    _ => {
                        panic!("Unexpected rule: {:?}", next.as_rule());
                    }
                };
            }
            Ok(v_flyweight(delegate, &slots, List::mk_list(&contents)))
        }
        _ => {
            panic!("Unimplemented literal: {pair:?}");
        }
    }
}

fn parse_object_literal(pair: Pair<Rule>) -> Result<Obj, ObjDefParseError> {
    match pair.as_rule() {
        Rule::object => {
            let ostr = &pair.as_str()[1..];
            let oid = i32::from_str(ostr).unwrap();
            let objid = Obj::mk_id(oid);
            Ok(objid)
        }
        _ => {
            panic!("Unexpected object literal: {pair:?}");
        }
    }
}

fn parse_string_literal(pair: Pair<Rule>) -> Result<String, ObjDefParseError> {
    let string = pair.as_str();
    let parsed = unquote_str(string).map_err(|e| {
        VerbCompileError(CompileError::StringLexError(
            CompileContext::new(pair.line_col()),
            e,
        ))
    })?;
    Ok(parsed)
}

fn parse_literal_atom(
    context: &mut ObjFileContext,
    pair: Pair<Rule>,
) -> Result<Var, ObjDefParseError> {
    match pair.as_rule() {
        Rule::object => {
            let objid = parse_object_literal(pair)?;
            Ok(v_obj(objid))
        }
        Rule::integer => Ok(v_int(
            pair.as_str()
                .parse::<i64>()
                .map_err(|e| {
                    CompileError::StringLexError(
                        CompileContext::new(pair.line_col()),
                        format!("Failed to parse '{}' to i64: {e}", pair.as_str()),
                    )
                })
                .map_err(VerbCompileError)?,
        )),
        Rule::float => Ok(v_float(
            pair.as_str()
                .parse::<f64>()
                .map_err(|e| {
                    CompileError::StringLexError(
                        CompileContext::new(pair.line_col()),
                        format!("Failed to parse '{}' to f64: {e}", pair.as_str()),
                    )
                })
                .map_err(VerbCompileError)?,
        )),
        Rule::string => {
            let str = parse_string_literal(pair)?;
            Ok(v_str(&str))
        }
        Rule::err => {
            let e = pair.as_str();
            let Some(e) = ErrorCode::parse_str(e) else {
                return Err(VerbCompileError(CompileError::ParseError {
                    error_position: CompileContext::new(pair.line_col()),
                    end_line_col: None,
                    context: e.to_string(),
                    message: e.to_string(),
                }));
            };
            Ok(v_err(e))
        }
        Rule::ident | Rule::variable => {
            let sym = Symbol::mk(pair.as_str());
            let Some(value) = context.0.get(&sym) else {
                return Err(ObjDefParseError::ConstantNotFound(sym.to_string()));
            };
            Ok(value.clone())
        }
        Rule::boolean => {
            let bool = pair.as_str() == "true";
            Ok(v_bool(bool))
        }
        _ => {
            panic!("Unimplemented atom: {pair:?}");
        }
    }
}

pub fn compile_object_definitions(
    objdef: &str,
    options: &CompileOptions,
    context: &mut ObjFileContext,
) -> Result<Vec<ObjectDefinition>, ObjDefParseError> {
    let mut pairs = match MooParser::parse(Rule::objects_file, objdef) {
        Ok(pairs) => pairs,
        Err(e) => {
            let ((line, column), end_line_col) = match e.line_col {
                LineColLocation::Pos(lc) => (lc, None),
                LineColLocation::Span(begin, end) => (begin, Some(end)),
            };

            return Err(VerbCompileError(CompileError::ParseError {
                error_position: CompileContext::new((line, column)),
                end_line_col,
                context: e.line().to_string(),
                message: e.variant.message().to_string(),
            }));
        }
    };

    let ofile = pairs.next().unwrap();
    let pairs = match ofile.as_rule() {
        Rule::objects_file => ofile.into_inner(),
        _ => {
            panic!("Expected object file, got {ofile:?}");
        }
    };

    let mut objdefs = vec![];
    for pair in pairs {
        match pair.as_rule() {
            Rule::object_definition => {
                objdefs.push(compile_object_definition(pair, options, context)?);
            }
            Rule::constant_decl => {
                let mut pairs = pair.into_inner();
                let constant = pairs.next().unwrap().as_str();
                let value = pairs.next().unwrap();
                let value = parse_literal(context, value)?;
                context.0.insert(Symbol::mk(constant), value);
            }
            Rule::EOI => {
                break;
            }
            _ => {
                panic!("Unexpected rule: {pair:?}");
            }
        }
    }

    Ok(objdefs)
}

fn parse_obj_attr(
    context: &mut ObjFileContext,
    inner: Pair<Rule>,
) -> Result<Obj, ObjDefParseError> {
    let value = parse_literal_atom(context, inner)?;
    let Some(obj) = value.as_object() else {
        return Err(ObjDefParseError::BadAttributeType(value.type_code()));
    };
    Ok(obj)
}

fn parse_str_attr(
    context: &mut ObjFileContext,
    inner: Pair<Rule>,
) -> Result<String, ObjDefParseError> {
    let value = parse_literal_atom(context, inner)?;
    let Some(name) = value.as_string() else {
        return Err(ObjDefParseError::BadAttributeType(value.type_code()));
    };
    Ok(name.to_string())
}

fn compile_object_definition(
    pair: Pair<Rule>,
    options: &CompileOptions,
    context: &mut ObjFileContext,
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
        property_definitions: Default::default(),
        property_overrides: Default::default(),
    };

    let mut pairs = pair.into_inner();
    let oid = parse_obj_attr(context, pairs.next().unwrap())?;
    objdef.oid = oid;

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
                        objdef.parent = parse_obj_attr(context, inner)?
                    }
                    Rule::name_attr => {
                        let inner = attr_pair.into_inner().next().unwrap();
                        objdef.name = parse_str_attr(context, inner)?
                    }
                    Rule::owner_attr => {
                        let inner = attr_pair.into_inner().next().unwrap();
                        objdef.owner = parse_obj_attr(context, inner)?
                    }
                    Rule::location_attr => {
                        let inner = attr_pair.into_inner().next().unwrap();
                        objdef.location = parse_obj_attr(context, inner)?
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
                        panic!("Unexpected object attribute: {attr_pair:?}");
                    }
                }
            }
        }
        _ => {
            panic!("Expected object attributes, got {object_attrs:?}");
        }
    }

    // Now it's either properties or verbs until end of program
    for pair in pairs {
        match pair.as_rule() {
            Rule::verb_decl => {
                let inner = pair.into_inner();
                let vd = parse_verb_decl(inner, options, context)?;
                objdef.verbs.push(vd);
            }
            Rule::prop_def => {
                let inner = pair.into_inner();
                let pd = parse_prop_def(context, inner)?;
                objdef.property_definitions.push(pd);
            }
            Rule::prop_set => {
                let inner = pair.into_inner();
                let ps = parse_prop_set(context, inner)?;
                objdef.property_overrides.push(ps);
            }
            Rule::EOI => {
                return Ok(objdef);
            }
            _ => {
                panic!("Unexpected rule: {pair:?}");
            }
        }
    }
    Ok(objdef)
}

fn parse_property_name(pair: Pair<Rule>) -> Result<Symbol, ObjDefParseError> {
    // If rule is "string", parse it as that. Otherwise, just grab the literal characters.
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::string => Ok(Symbol::mk(parse_string_literal(inner)?.as_str())),
        Rule::unquoted_propname => Ok(Symbol::mk(inner.as_str().trim())),
        _ => {
            panic!("Unexpected property name: {inner:?}");
        }
    }
}

fn parse_prop_set(
    context: &mut ObjFileContext,
    mut pairs: Pairs<Rule>,
) -> Result<ObjPropOverride, ObjDefParseError> {
    let name = parse_property_name(pairs.next().unwrap())?;

    // Next is either propinfo -> perms override, or if not, it's straight to the value.
    // So check what it is first
    let mut next = pairs.next();
    let pr = next.clone().unwrap();
    let perms_update = if pr.as_rule() == Rule::propinfo {
        // owner, flags
        let mut inner = pr.into_inner();
        let attr_pair = inner.next().unwrap();
        let attr_inner = attr_pair.into_inner().next().unwrap();
        let owner = parse_obj_attr(context, attr_inner)?;

        let flags_pair = inner.next().unwrap();
        let inner = flags_pair.into_inner();
        let flags_str = inner.as_str();
        let flags_str = flags_str.replace('"', "");
        let Some(flags) = PropFlag::parse_str(&flags_str) else {
            return Err(ObjDefParseError::BadPropFlags(flags_str.to_string()));
        };
        next = pairs.next();
        Some(PropPerms::new(owner, flags))
    } else {
        None
    };

    let value = match &next {
        Some(n) => match n.as_rule() {
            Rule::literal => {
                let inner = n.clone().into_inner().next().unwrap();
                Some(parse_literal(context, inner)?)
            }
            _ => {
                panic!("Expected literal, got {next:?}");
            }
        },
        None => None,
    };

    Ok(ObjPropOverride {
        name,
        perms_update,
        value,
    })
}

fn parse_prop_def(
    context: &mut ObjFileContext,
    mut pairs: Pairs<Rule>,
) -> Result<ObjPropDef, ObjDefParseError> {
    let name = parse_property_name(pairs.next().unwrap())?;

    let propinfo = pairs.next().unwrap();
    let perms = match propinfo.as_rule() {
        Rule::propinfo => {
            // owner, flags
            let mut inner = propinfo.into_inner();
            let attr_pair = inner.next().unwrap();
            let attr_inner = attr_pair.into_inner().next().unwrap();
            let owner = parse_obj_attr(context, attr_inner)?;

            let flags_pair = inner.next().unwrap();
            let inner = flags_pair.into_inner();
            let flags_str = inner.as_str();
            let flags_str = flags_str.replace('"', "");
            let Some(flags) = PropFlag::parse_str(&flags_str) else {
                return Err(ObjDefParseError::BadPropFlags(flags_str.to_string()));
            };
            PropPerms::new(owner, flags)
        }
        _ => {
            panic!("Expected propflags, got {propinfo:?}");
        }
    };

    let value = match pairs.next() {
        None => None,
        Some(pair) => match pair.as_rule() {
            Rule::literal => {
                let inner = pair.into_inner().next().unwrap();
                Some(parse_literal(context, inner)?)
            }
            _ => {
                panic!("Expected literal, got {pair:?}");
            }
        },
    };

    Ok(ObjPropDef { name, perms, value })
}

fn parse_verb_decl(
    mut pairs: Pairs<Rule>,
    compile_options: &CompileOptions,
    context: &mut ObjFileContext,
) -> Result<ObjVerbDef, ObjDefParseError> {
    // First is the verb_names
    let mut vd = ObjVerbDef {
        names: vec![],
        argspec: VerbArgsSpec::this_none_this(),
        owner: NOTHING,
        flags: VerbFlag::r(),
        program: ProgramType::MooR(Default::default()),
    };

    let verb_names = pairs.next().unwrap();
    let verb_names = verb_names.into_inner().next().unwrap();

    // verb names have to be parsed as a string.
    // And then we split on spaces.
    match verb_names.as_rule() {
        Rule::string => {
            let verb_names = parse_string_literal(verb_names)?;
            let verb_names = verb_names.split_whitespace();
            for name in verb_names {
                let name = Symbol::mk(name.trim());
                vd.names.push(name);
            }
        }
        _ => {
            let verb_name = verb_names.as_str().trim();
            let name = Symbol::mk(verb_name);
            vd.names.push(name);
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
            panic!("Expected verb argspec, got {argspec_pair:?}");
        }
    }

    // Now owner & flags
    let owner_pair = pairs.next().unwrap();
    match owner_pair.as_rule() {
        Rule::owner_attr => {
            let inner = owner_pair.into_inner().next().unwrap();
            vd.owner = parse_obj_attr(context, inner)?;
        }
        _ => {
            panic!("Expected owner, got {owner_pair:?}");
        }
    }

    // Then flags, which is a string like 'rxd' etc.
    let flags_pair = pairs.next().unwrap();
    match flags_pair.as_rule() {
        Rule::flags_attr => {
            let inner = flags_pair.into_inner();
            let flags_str = inner.as_str();
            let flags_str = flags_str.replace('"', "");
            let Some(flags) = VerbFlag::parse_str(&flags_str) else {
                return Err(ObjDefParseError::BadVerbFlags(flags_str.to_string()));
            };
            vd.flags = flags;
        }
        _ => {
            panic!("Expected flags, got {flags_pair:?}");
        }
    }

    // Now the verb body, which we will attempt to compile with a specialized form of the regular
    // compiler.
    let verb_body = pairs.next().unwrap();
    let program = match verb_body.as_rule() {
        Rule::verb_statements => {
            let inner = verb_body.into_inner();
            compile_tree(inner, compile_options.clone()).map_err(VerbCompileError)?
        }
        _ => {
            panic!("Expected verb body, got {verb_body:?}");
        }
    };

    // Encode the program
    vd.program = ProgramType::MooR(program);

    Ok(vd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_common::matching::Preposition;
    use moor_var::{E_INVIND, v_err};

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

        let mut context = ObjFileContext::new();
        let odef =
            &compile_object_definitions(spec, &CompileOptions::default(), &mut context).unwrap()[0];
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

                    verb "look_self look_*" (this to any) owner: #2 flags: "rxd"
                        return 5;
                    endverb

                    verb another_test (this none this) owner: #2 flags: "r"
                        player:tell("here is something");
                    endverb
                endobject"#;
        let mut context = ObjFileContext::new();
        let odef =
            &compile_object_definitions(spec, &CompileOptions::default(), &mut context).unwrap()[0];

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

        assert_eq!(odef.verbs[1].names.len(), 1);
        assert_eq!(odef.verbs[1].names[0], Symbol::mk("another_test"));
        assert_eq!(odef.verbs[1].argspec.dobj, ArgSpec::This);
        assert_eq!(odef.verbs[1].argspec.prep, PrepSpec::None);
        assert_eq!(odef.verbs[1].argspec.iobj, ArgSpec::This);
        assert_eq!(odef.verbs[1].owner, Obj::mk_id(2));
        assert_eq!(odef.verbs[1].flags, VerbFlag::r());
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

                    // Declares a property named "description" with owner #2 and flags "rc"
                    property description (owner: #2, flags: "rc") = "This is a test object";
                    // empty flags are also permitted
                    property other (owner: #2, flags: "");
                endobject"#;

        let mut context = ObjFileContext::new();
        let odef =
            &compile_object_definitions(spec, &CompileOptions::default(), &mut context).unwrap()[0];

        assert_eq!(odef.property_definitions.len(), 2);
        assert_eq!(odef.property_definitions[0].name, Symbol::mk("description"));
        assert_eq!(odef.property_definitions[0].perms.owner(), Obj::mk_id(2));
        assert_eq!(odef.property_definitions[0].perms.flags(), PropFlag::rc());
        let Some(s) = odef.property_definitions[0]
            .value
            .as_ref()
            .unwrap()
            .as_string()
        else {
            panic!("Expected string value");
        };
        assert_eq!(s, "This is a test object");

        assert_eq!(odef.property_definitions[1].name, Symbol::mk("other"));
        assert_eq!(odef.property_definitions[1].perms.owner(), Obj::mk_id(2));
        assert_eq!(odef.property_definitions[1].perms.flags(), BitEnum::new());
        assert!(odef.property_definitions[1].value.is_none());
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
                    override other (owner: #2, flags: "rc") = "test";
                endobject"#;
        let mut context = ObjFileContext::new();
        let odef =
            &compile_object_definitions(spec, &CompileOptions::default(), &mut context).unwrap()[0];

        assert_eq!(odef.property_overrides.len(), 2);
        assert_eq!(odef.property_overrides[0].name, Symbol::mk("description"));
        let Some(s) = odef.property_overrides[0]
            .value
            .as_ref()
            .unwrap()
            .as_string()
        else {
            panic!("Expected string value");
        };
        assert_eq!(s, "This is a test object");

        assert_eq!(odef.property_overrides[1].name, Symbol::mk("other"));
        let Some(s) = odef.property_overrides[1]
            .value
            .as_ref()
            .unwrap()
            .as_string()
        else {
            panic!("Expected string value");
        };
        assert_eq!(s, "test");
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
                    property description (owner: #2, flags: "rc") = "This is a test object";
                    property other (owner: #2, flags: "rc");
                    override description = "This is a test object";
                    override other (owner: #2, flags: "rc") = "test";

                    override "@funky_prop_name" = "test";
                    
                    verb "look_self look_*" (this to any) owner: #2 flags: "rxd"
                        return 5;
                    endverb

                    verb another_test (this none this) owner: #2 flags: "r"
                        player:tell("here is something");
                    endverb


                endobject"#;
        let mut context = ObjFileContext::new();
        compile_object_definitions(spec, &CompileOptions::default(), &mut context).unwrap();
    }

    #[test]
    fn test_various_literals() {
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

                    override objid = #-1;
                    override string = "Hello World";
                    override integer = 12345;
                    override float = 123.45;
                    override error = E_INVIND;
                    override list = {1, 2, 3, 4};
                    override map = [ 1 -> 2, "test" -> 4 ];
                    override nested_list = { 1,2, { 5, 6, 7 }};
                    override nested_map = [ 1 -> [ 2 -> 3, 4 -> 5 ], 6 -> 7 ];
                    override flyweight = <#1, [ a -> 1, b-> 2 ], { 1,2, 3}>;
                endobject"#;
        let mut context = ObjFileContext::new();
        let odef =
            &compile_object_definitions(spec, &CompileOptions::default(), &mut context).unwrap();

        assert_eq!(
            odef[0].property_overrides[0]
                .value
                .as_ref()
                .unwrap()
                .clone(),
            v_obj(NOTHING)
        );
        assert_eq!(
            odef[0].property_overrides[1]
                .value
                .as_ref()
                .unwrap()
                .clone(),
            v_str("Hello World")
        );
        assert_eq!(
            odef[0].property_overrides[2]
                .value
                .as_ref()
                .unwrap()
                .clone(),
            v_int(12345)
        );
        assert_eq!(
            odef[0].property_overrides[3]
                .value
                .as_ref()
                .unwrap()
                .clone(),
            v_float(123.45)
        );
        assert_eq!(
            odef[0].property_overrides[4]
                .value
                .as_ref()
                .unwrap()
                .clone(),
            v_err(E_INVIND)
        );
        assert_eq!(
            odef[0].property_overrides[5]
                .value
                .as_ref()
                .unwrap()
                .clone(),
            v_list(&[v_int(1), v_int(2), v_int(3), v_int(4)])
        );
        assert_eq!(
            odef[0].property_overrides[6]
                .value
                .as_ref()
                .unwrap()
                .clone(),
            v_map(&[(v_int(1), v_int(2)), (v_str("test"), v_int(4))])
        );
        assert_eq!(
            odef[0].property_overrides[7]
                .value
                .as_ref()
                .unwrap()
                .clone(),
            v_list(&[v_int(1), v_int(2), v_list(&[v_int(5), v_int(6), v_int(7)])])
        );
        assert_eq!(
            odef[0].property_overrides[8]
                .value
                .as_ref()
                .unwrap()
                .clone(),
            v_map(&[
                (
                    v_int(1),
                    v_map(&[(v_int(2), v_int(3)), (v_int(4), v_int(5))])
                ),
                (v_int(6), v_int(7))
            ])
        );
        assert_eq!(
            odef[0].property_overrides[9]
                .value
                .as_ref()
                .unwrap()
                .clone(),
            v_flyweight(
                Obj::mk_id(1),
                &[(Symbol::mk("a"), v_int(1)), (Symbol::mk("b"), v_int(2))],
                List::mk_list(&[v_int(1), v_int(2), v_int(3)]),
            )
        );
    }

    #[test]
    fn test_exotic_verbnames() {
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

                    verb "@building-o*ptions @buildingo*ptions" (any any any) owner: #184 flags: "rd"
                        return 5;
                    endverb

                    verb "modname_# @recycle!" (any any any) owner: #184 flags: "rd"
                        return 5;
                    endverb

                    verb "contains_\"quote" (this none this) owner: #184 flags: "rxd"
                        player:tell("here is something");
                    endverb
                endobject"#;
        let mut context = ObjFileContext::new();
        compile_object_definitions(spec, &CompileOptions::default(), &mut context).unwrap();
    }

    #[test]
    fn test_constants_usage() {
        let spec = r#"
                define ROOT = #1;
                define SYS_OBJ = #2;
                define MAGIC = "magic constant";
                define NESTED = { ROOT, SYS_OBJ, MAGIC };

                object #1
                    parent: ROOT
                    name: MAGIC
                    location: SYS_OBJ
                    wizard: false
                    programmer: false
                    player: false
                    fertile: true
                    readable: true

                    property description (owner: SYS_OBJ, flags: "rc") = "This is a test object";

                    verb "contains_\"quote" (this none this) owner: ROOT flags: "rxd"
                        player:tell("here is something");
                    endverb
                endobject"#;

        let mut context = ObjFileContext::new();
        compile_object_definitions(spec, &CompileOptions::default(), &mut context).unwrap();
    }

    /// Regression on quoted string constants for property names
    #[test]
    fn test_quoted_string_propname() {
        let spec = r#"object #1
                    // Testing a C++ Style comment
                    parent: #1
                    name: "Test Object"
                    location: #3
                    wizard: false
                    programmer: false
                    player: false
                    fertile: true
                    readable: true

                    property " " (owner: #1, flags: "") = {"", "", {}, {}};
                    property just_regular (owner: #1, flags: "") = {"", "", {}, {}};
                    property "quoted normal" (owner: #1, flags: "") = {"", "", {}, {}};
                endobject"#;

        let mut context = ObjFileContext::new();
        let objs =
            compile_object_definitions(spec, &CompileOptions::default(), &mut context).unwrap();
        let obj = &objs[0];

        assert_eq!(obj.property_definitions[0].name, Symbol::mk(" "));
        assert_eq!(obj.property_definitions[1].name, Symbol::mk("just_regular"));
        assert_eq!(
            obj.property_definitions[2].name,
            Symbol::mk("quoted normal")
        );
    }
}
