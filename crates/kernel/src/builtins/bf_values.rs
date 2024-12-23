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

use crate::bf_declare;
use crate::builtins::BfRet::Ret;
use crate::builtins::{world_state_bf_err, BfCallState, BfErr, BfRet, BuiltinFunction};
use md5::Digest;
use moor_compiler::{offset_for_builtin, to_literal};
use moor_values::model::WorldState;
use moor_values::Error::{E_ARGS, E_INVARG, E_INVIND, E_PERM, E_RANGE, E_TYPE};
use moor_values::{
    v_bool, v_float, v_int, v_list, v_obj, v_objid, v_str, v_string, Flyweight, List, Map, Obj,
};
use moor_values::{v_flyweight, Associative};
use moor_values::{AsByteBuffer, Sequence};
use moor_values::{Symbol, Variant, SYSTEM_OBJECT};
use std::io::{BufReader, BufWriter};
use tracing::error;
use xml::reader::XmlEvent;
use xml::EmitterConfig;

fn bf_typeof(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    let arg = &bf_args.args[0];
    Ok(Ret(v_int(arg.type_code() as i64)))
}
bf_declare!(typeof, bf_typeof);

fn bf_tostr(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    let mut result = String::new();
    for arg in bf_args.args.iter() {
        match arg.variant() {
            Variant::None => result.push_str("None"),
            Variant::Int(i) => result.push_str(&i.to_string()),
            Variant::Float(f) => result.push_str(format!("{:?}", f).as_str()),
            Variant::Str(s) => result.push_str(s.as_string().as_str()),
            Variant::Obj(o) => result.push_str(&o.to_string()),
            Variant::List(_) => result.push_str("{list}"),
            Variant::Map(_) => result.push_str("[map]"),
            Variant::Err(e) => result.push_str(e.name()),
            Variant::Flyweight(fl) => {
                if fl.is_sealed() {
                    result.push_str("<sealed flyweight>")
                } else {
                    result.push_str("<flyweight>")
                }
            }
        }
    }
    Ok(Ret(v_str(result.as_str())))
}
bf_declare!(tostr, bf_tostr);

fn bf_toliteral(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }
    let literal = to_literal(&bf_args.args[0]);
    Ok(Ret(v_str(literal.as_str())))
}
bf_declare!(toliteral, bf_toliteral);

fn bf_toint(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }
    match bf_args.args[0].variant() {
        Variant::Int(i) => Ok(Ret(v_int(*i))),
        Variant::Float(f) => Ok(Ret(v_int(*f as i64))),
        Variant::Obj(o) => Ok(Ret(v_int(o.id().0 as i64))),
        Variant::Str(s) => {
            let i = s.as_string().as_str().parse::<f64>();
            match i {
                Ok(i) => Ok(Ret(v_int(i as i64))),
                Err(_) => Ok(Ret(v_int(0))),
            }
        }
        Variant::Err(e) => Ok(Ret(v_int(*e as i64))),
        _ => Err(BfErr::Code(E_INVARG)),
    }
}
bf_declare!(toint, bf_toint);

fn bf_toobj(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }
    match bf_args.args[0].variant() {
        Variant::Int(i) => {
            let i = if *i < i32::MIN as i64 || *i > i32::MAX as i64 {
                return Err(BfErr::Code(E_RANGE));
            } else {
                *i as i32
            };
            Ok(Ret(v_objid(i)))
        }
        Variant::Float(f) => {
            let f = if *f < i32::MIN as f64 || *f > i32::MAX as f64 {
                return Err(BfErr::Code(E_RANGE));
            } else {
                *f as i32
            };
            Ok(Ret(v_objid(f)))
        }
        Variant::Str(s) if s.as_string().as_str().starts_with('#') => {
            let i = s.as_string().as_str()[1..].parse::<i32>();
            match i {
                Ok(i) => Ok(Ret(v_objid(i))),
                Err(_) => Ok(Ret(v_objid(0))),
            }
        }
        Variant::Str(s) => {
            let i = s.as_string().as_str().parse::<i32>();
            match i {
                Ok(i) => Ok(Ret(v_objid(i))),
                Err(_) => Ok(Ret(v_objid(0))),
            }
        }
        _ => Err(BfErr::Code(E_INVARG)),
    }
}
bf_declare!(toobj, bf_toobj);

fn bf_tofloat(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }
    match bf_args.args[0].variant() {
        Variant::Int(i) => Ok(Ret(v_float(*i as f64))),
        Variant::Float(f) => Ok(Ret(v_float(*f))),
        Variant::Str(s) => {
            let f = s.as_string().as_str().parse::<f64>();
            match f {
                Ok(f) => Ok(Ret(v_float(f))),
                Err(_) => Ok(Ret(v_float(0.0))),
            }
        }
        Variant::Err(e) => Ok(Ret(v_float(*e as u8 as f64))),
        _ => Err(BfErr::Code(E_INVARG)),
    }
}
bf_declare!(tofloat, bf_tofloat);

fn bf_equal(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }
    let (a1, a2) = (&bf_args.args[0], &bf_args.args[1]);
    let result = a1.eq_case_sensitive(a2);
    Ok(Ret(v_bool(result)))
}
bf_declare!(equal, bf_equal);

fn bf_value_bytes(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }
    let count = bf_args.args[0].size_bytes();
    Ok(Ret(v_int(count as i64)))
}
bf_declare!(value_bytes, bf_value_bytes);

fn bf_value_hash(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }
    let s = to_literal(&bf_args.args[0]);
    let hash_digest = md5::Md5::digest(s.as_bytes());
    Ok(Ret(v_str(
        format!("{:x}", hash_digest).to_uppercase().as_str(),
    )))
}
bf_declare!(value_hash, bf_value_hash);

fn bf_length(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    match bf_args.args[0].variant() {
        Variant::Str(s) => Ok(Ret(v_int(s.len() as i64))),
        Variant::List(l) => Ok(Ret(v_int(l.len() as i64))),
        Variant::Map(m) => Ok(Ret(v_int(m.len() as i64))),
        _ => Err(BfErr::Code(E_TYPE)),
    }
}
bf_declare!(length, bf_length);

fn bf_object_bytes(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }
    let Variant::Obj(o) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_INVARG));
    };
    if !bf_args.world_state.valid(o).map_err(world_state_bf_err)? {
        return Err(BfErr::Code(E_INVARG));
    };
    let size = bf_args
        .world_state
        .object_bytes(&bf_args.caller_perms(), o)
        .map_err(world_state_bf_err)?;
    Ok(Ret(v_int(size as i64)))
}
bf_declare!(object_bytes, bf_object_bytes);

/// Uses xml-rs to parse a string into a series of flyweights
/// representing the XML structure.
/// Delegates for the flyweights are resolved as follows:
/// a) For each tag, there should be an object: $tag_<tag> for that tag name,
/// b) Alternatively, if a map is provided as the second argument, the tag name is looked up
///    in the map, and the object is resolved from that.
fn bf_xml_parse(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.config.flyweight_type {
        return Err(BfErr::Code(E_PERM));
    }

    if bf_args.args.len() != 1 && bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }

    let Variant::Str(xml) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_INVARG));
    };

    let map = if bf_args.args.len() == 2 {
        let Variant::Map(m) = bf_args.args[1].variant() else {
            return Err(BfErr::Code(E_INVARG));
        };
        Some(m)
    } else {
        None
    };

    let reader = BufReader::new(xml.as_string().as_bytes());
    let parser = xml::EventReader::new(reader);
    let mut output_tree = Vec::new();

    // Structure is (tag, Vec<(attribute, value)>, Vec<...>)
    let mut current_node = Vec::new();
    for e in parser {
        match e {
            Ok(XmlEvent::StartElement {
                name, attributes, ..
            }) => {
                let tag = name.local_name;
                let obj = match map {
                    Some(m) => {
                        let key = tag.to_string();
                        let key = v_str(key.as_str());
                        let Ok(obj) = m.index(&key) else {
                            return Err(BfErr::Code(E_INVARG));
                        };
                        let Variant::Obj(o) = obj.variant() else {
                            return Err(BfErr::Code(E_TYPE));
                        };
                        o.clone()
                    }
                    None => {
                        let key = format!("tag_{}", tag);
                        let key = Symbol::mk(&key);

                        // resolve via system object
                        let prop_value = bf_args
                            .world_state
                            .retrieve_property(&bf_args.caller_perms(), &SYSTEM_OBJECT, key)
                            .map_err(world_state_bf_err)?;

                        let Variant::Obj(o) = prop_value.variant() else {
                            return Err(BfErr::Code(E_TYPE));
                        };

                        o.clone()
                    }
                };

                let attributes: Vec<_> = attributes
                    .iter()
                    .map(|a| {
                        let key = format!("{}", a.name);
                        let key = Symbol::mk(&key);
                        let value = v_str(a.value.as_str());
                        (key, value)
                    })
                    .collect();
                let entry = (obj, attributes, Vec::new());
                current_node.push(entry);
            }
            Ok(XmlEvent::EndElement { .. }) => {
                let (obj, attributes, children) =
                    current_node.pop().ok_or(BfErr::Code(E_INVARG))?;
                // Turn this into a flyweight and push into the children of the parent
                let children = List::mk_list(&children);
                let fl = v_flyweight(obj.clone(), &attributes, children, None);
                if let Some(parent) = current_node.last_mut() {
                    parent.2.push(fl);
                } else {
                    output_tree.push(fl);
                }
            }
            Ok(XmlEvent::Characters(str)) => {
                // Character data between tags is stored a String in the parent's content
                if let Some(parent) = current_node.last_mut() {
                    parent.2.push(v_str(str.as_str()));
                }
            }
            Ok(_) => {
                // Ignore other events (CDATA, etc)
            }
            Err(_) => {
                return Err(BfErr::Code(E_INVARG));
            }
        }
    }

    // Return output tree as a v_list.
    let result = v_list(&output_tree);
    Ok(Ret(result))
}
bf_declare!(xml_parse, bf_xml_parse);

enum Tag {
    StartElement(String, Vec<(String, String)>),
    EndElement(()),
    Text(String),
}

fn flyweight_to_xml_tag(
    fl: &Flyweight,
    map: Option<&Map>,
    perms: &Obj,
    ws: &mut dyn WorldState,
) -> Result<Vec<Tag>, BfErr> {
    let mut tags = Vec::new();

    // tag name can be derived by either looking in the optionally provided map, or by
    // seeking a `tag` property on the delegate object.
    let tag_name = match map {
        Some(m) => {
            let key = v_obj(fl.delegate().clone());
            let Ok(tag) = m.index(&key) else {
                return Err(BfErr::Code(E_INVARG));
            };
            let Variant::Str(s) = tag.variant() else {
                return Err(BfErr::Code(E_INVARG));
            };
            s.as_string().to_string()
        }
        None => {
            let key = Symbol::mk("tag");
            let tag = ws
                .retrieve_property(perms, fl.delegate(), key)
                .map_err(world_state_bf_err)?;

            let Variant::Str(s) = tag.variant() else {
                return Err(BfErr::Code(E_TYPE));
            };

            s.as_string().to_string()
        }
    };

    let mut attributes = Vec::with_capacity(fl.slots().len());
    for (key, value) in fl.slots() {
        let key = key.to_string();
        let value = match value.variant() {
            Variant::Str(s) => s.as_string().to_string(),
            Variant::Int(i) => i.to_string(),
            Variant::Float(f) => f.to_string(),
            _ => {
                error!("Invalid attribute type");
                return Err(BfErr::Code(E_INVARG));
            }
        };
        attributes.push((key, value));
    }

    tags.push(Tag::StartElement(tag_name, attributes));

    for child in fl.contents().iter() {
        match child.variant() {
            Variant::Flyweight(fl) => {
                let child_tags = flyweight_to_xml_tag(fl, map, perms, ws)?;
                tags.extend(child_tags);
            }
            Variant::Str(s) => {
                tags.push(Tag::Text(s.as_string().to_string()));
            }
            _ => {
                error!("Invalid child type");
                return Err(BfErr::Code(E_INVARG));
            }
        }
    }
    tags.push(Tag::EndElement(()));

    Ok(tags)
}

/// to_xml(root_flyweight, [tag map]) -> string
///
/// Turn a tree of flyweights into an XML document.
/// Valid flyweights must have:
///  - delegate object with a tag property OR there's a second map argument that maps object ids to tags
///  - attributes property that is a map of strings to string or numbers
///  - any children must be either other valid flyweights, or string values.
fn bf_to_xml(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.config.flyweight_type {
        return Err(BfErr::Code(E_PERM));
    }

    if bf_args.args.len() != 1 && bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }

    let root = &bf_args.args[0];
    let map = if bf_args.args.len() == 2 {
        let Variant::Map(m) = bf_args.args[1].variant() else {
            return Err(BfErr::Code(E_INVARG));
        };
        Some(m)
    } else {
        None
    };

    let mut output = Vec::new();
    {
        let mut output_buf = BufWriter::new(&mut output);
        let mut writer = EmitterConfig::new()
            .perform_indent(true)
            .create_writer(&mut output_buf);

        // Element needs to be a flyweight
        let Variant::Flyweight(fl) = root.variant() else {
            return Err(BfErr::Code(E_INVARG));
        };

        let root_tag = flyweight_to_xml_tag(fl, map, &bf_args.caller_perms(), bf_args.world_state)?;
        for tag in root_tag {
            match tag {
                Tag::StartElement(name, attributes) => {
                    let element_builder = xml::writer::XmlEvent::start_element(name.as_str());
                    let element_builder =
                        attributes.iter().fold(element_builder, |builder, (k, v)| {
                            builder.attr(k.as_str(), v.as_str())
                        });
                    writer
                        .write(element_builder)
                        .map_err(|_| BfErr::Code(E_INVIND))?;
                }
                Tag::Text(text) => {
                    writer
                        .write(xml::writer::XmlEvent::characters(text.as_str()))
                        .map_err(|_| BfErr::Code(E_INVIND))?;
                }
                Tag::EndElement(_) => {
                    writer
                        .write(xml::writer::XmlEvent::end_element())
                        .map_err(|_| BfErr::Code(E_INVIND))?;
                }
            }
        }
    }
    let output_as_string = String::from_utf8(output).map_err(|_| BfErr::Code(E_INVIND))?;
    Ok(Ret(v_string(output_as_string)))
}
bf_declare!(to_xml, bf_to_xml);

pub(crate) fn register_bf_values(builtins: &mut [Box<dyn BuiltinFunction>]) {
    builtins[offset_for_builtin("typeof")] = Box::new(BfTypeof {});
    builtins[offset_for_builtin("tostr")] = Box::new(BfTostr {});
    builtins[offset_for_builtin("toliteral")] = Box::new(BfToliteral {});
    builtins[offset_for_builtin("toint")] = Box::new(BfToint {});
    builtins[offset_for_builtin("tonum")] = Box::new(BfToint {});
    builtins[offset_for_builtin("toobj")] = Box::new(BfToobj {});
    builtins[offset_for_builtin("tofloat")] = Box::new(BfTofloat {});
    builtins[offset_for_builtin("equal")] = Box::new(BfEqual {});
    builtins[offset_for_builtin("value_bytes")] = Box::new(BfValueBytes {});
    builtins[offset_for_builtin("object_bytes")] = Box::new(BfObjectBytes {});
    builtins[offset_for_builtin("value_hash")] = Box::new(BfValueHash {});
    builtins[offset_for_builtin("length")] = Box::new(BfLength {});

    // Extensions...
    builtins[offset_for_builtin("xml_parse")] = Box::new(BfXmlParse {});
    builtins[offset_for_builtin("to_xml")] = Box::new(BfToXml {});
}
