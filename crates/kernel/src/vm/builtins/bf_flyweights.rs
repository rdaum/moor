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

use crate::vm::builtins::BfRet::Ret;
use crate::vm::builtins::{BfCallState, BfErr, BfRet, BuiltinFunction, world_state_bf_err};
use moor_common::model::WorldState;
use moor_compiler::offset_for_builtin;
use moor_var::{
    Associative, E_ARGS, E_INVARG, E_INVIND, E_PERM, E_TYPE, Flyweight, List, Map, Obj,
    SYSTEM_OBJECT, Sequence, Symbol, Variant, v_flyweight, v_list, v_map, v_obj, v_str, v_string,
    v_sym,
};
use std::io::{BufReader, BufWriter};
use tracing::error;
use xml::EmitterConfig;
use xml::reader::XmlEvent;

/// Uses xml-rs to parse a string into a series of flyweights
/// representing the XML structure.
/// Delegates for the flyweights are resolved as follows:
/// a) For each tag, there should be an object: $tag_<tag> for that tag name,
/// b) Alternatively, if a map is provided as the second argument, the tag name is looked up
///    in the map, and the object is resolved from that.
fn bf_xml_parse(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.config.flyweight_type {
        return Err(BfErr::ErrValue(E_PERM.msg("Flyweights not enabled")));
    }

    if bf_args.args.len() != 1 && bf_args.args.len() != 2 {
        return Err(BfErr::ErrValue(E_ARGS.with_msg(|| {
            format!(
                "xml_parse() takes 1 or 2 arguments, got {}",
                bf_args.args.len()
            )
        })));
    }

    let Variant::Str(xml) = bf_args.args[0].variant() else {
        return Err(BfErr::ErrValue(E_INVARG.with_msg(|| {
            format!(
                "xml_parse() expects a string argument, got {}",
                bf_args.args[0].type_code().to_literal()
            )
        })));
    };

    let map = if bf_args.args.len() == 2 {
        let Variant::Map(m) = bf_args.args[1].variant() else {
            return Err(BfErr::ErrValue(E_INVARG.with_msg(|| {
                format!(
                    "xml_parse() expects a map as the second argument, got {}",
                    bf_args.args[1].type_code().to_literal()
                )
            })));
        };
        Some(m)
    } else {
        None
    };

    let reader = BufReader::new(xml.as_str().as_bytes());
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
                        let Ok(obj) = m.get(&key) else {
                            return Err(BfErr::ErrValue(E_INVARG.with_msg(|| {
                                format!("xml_parse() tag {} not found in map", tag)
                            })));
                        };
                        let Variant::Obj(o) = obj.variant() else {
                            return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
                                format!("xml_parse() tag {} in map is not an object", tag)
                            })));
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
                            return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
                                format!("xml_parse() tag {} not found in system object", tag)
                            })));
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
                let (obj, attributes, children) = current_node.pop().ok_or_else(|| {
                    BfErr::ErrValue(
                        E_INVARG.with_msg(|| "xml_parse() end tag without start tag".to_string()),
                    )
                })?;
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
            Err(e) => {
                return Err(BfErr::ErrValue(
                    E_INVARG.with_msg(|| format!("xml_parse() error parsing XML: {}", e)),
                ));
            }
        }
    }

    // Return output tree as a v_list.
    let result = v_list(&output_tree);
    Ok(Ret(result))
}

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
            let Ok(tag) = m.get(&key) else {
                return Err(BfErr::ErrValue(E_INVARG.with_msg(|| {
                    format!("to_xml() tag {} not found in map", fl.delegate().id())
                })));
            };
            let Variant::Str(s) = tag.variant() else {
                return Err(BfErr::ErrValue(E_INVARG.with_msg(|| {
                    format!("to_xml() tag {} in map is not a string", fl.delegate().id())
                })));
            };
            s.as_str().to_string()
        }
        None => {
            let key = Symbol::mk("tag");
            let tag = ws
                .retrieve_property(perms, fl.delegate(), key)
                .map_err(world_state_bf_err)?;

            let Variant::Str(s) = tag.variant() else {
                return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
                    format!("to_xml() tag {} is not a string", fl.delegate().id())
                })));
            };

            s.as_str().to_string()
        }
    };

    let mut attributes = Vec::with_capacity(fl.slots().len());
    for (key, value) in fl.slots() {
        let key = key.to_string();
        let value = match value.variant() {
            Variant::Str(s) => s.as_str().to_string(),
            Variant::Int(i) => i.to_string(),
            Variant::Float(f) => f.to_string(),
            _ => {
                error!("Invalid attribute type");
                return Err(BfErr::ErrValue(E_INVARG.with_msg(|| {
                    format!(
                        "to_xml() attribute {} is not a string or number (is {})",
                        key,
                        value.type_code().to_literal()
                    )
                })));
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
                tags.push(Tag::Text(s.as_str().to_string()));
            }
            _ => {
                return Err(BfErr::ErrValue(
                    E_INVARG.msg("to_xml() child is not a flyweight or string"),
                ));
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
        return Err(BfErr::ErrValue(E_PERM.msg("Flyweights not enabled")));
    }

    if bf_args.args.len() != 1 && bf_args.args.len() != 2 {
        return Err(BfErr::ErrValue(E_ARGS.with_msg(|| {
            format!(
                "to_xml() takes 1 or 2 arguments, got {}",
                bf_args.args.len()
            )
        })));
    }

    let root = &bf_args.args[0];
    let map = if bf_args.args.len() == 2 {
        let Variant::Map(m) = bf_args.args[1].variant() else {
            return Err(BfErr::ErrValue(E_INVARG.with_msg(|| {
                format!(
                    "to_xml() expects a map as the second argument, got {}",
                    bf_args.args[1].type_code().to_literal()
                )
            })));
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
            return Err(BfErr::ErrValue(E_INVARG.with_msg(|| {
                format!(
                    "to_xml() expects a flyweight as the first argument, got {}",
                    root.type_code().to_literal()
                )
            })));
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
                    writer.write(element_builder).map_err(|e| {
                        BfErr::ErrValue(
                            E_INVIND.with_msg(|| format!("to_xml() error writing XML: {}", e)),
                        )
                    })?;
                }
                Tag::Text(text) => {
                    writer
                        .write(xml::writer::XmlEvent::characters(text.as_str()))
                        .map_err(|e| {
                            BfErr::ErrValue(
                                E_INVIND.with_msg(|| format!("to_xml() error writing XML: {}", e)),
                            )
                        })?;
                }
                Tag::EndElement(_) => {
                    writer
                        .write(xml::writer::XmlEvent::end_element())
                        .map_err(|e| {
                            BfErr::ErrValue(
                                E_INVIND.with_msg(|| format!("to_xml() error writing XML: {}", e)),
                            )
                        })?;
                }
            }
        }
    }
    let output_as_string = String::from_utf8(output).map_err(|e| {
        BfErr::ErrValue(
            E_INVIND.with_msg(|| format!("to_xml() error converting XML to string: {}", e)),
        )
    })?;
    Ok(Ret(v_string(output_as_string)))
}

/// slots(flyweight) - returns the set of slots on the flyweight as a map
fn bf_slots(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.config.flyweight_type {
        return Err(BfErr::ErrValue(E_PERM.msg("Flyweights not enabled")));
    }

    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.with_msg(|| {
            format!("slots() takes 1 argument, got {}", bf_args.args.len())
        })));
    }

    let Variant::Flyweight(f) = bf_args.args[0].variant() else {
        return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
            format!(
                "slots() expects a flyweight as the first argument, got {}",
                bf_args.args[0].type_code().to_literal()
            )
        })));
    };

    let slots: Vec<_> = f
        .slots()
        .iter()
        .map(|(k, v)| (v_sym(*k), v.clone()))
        .collect();
    let map = v_map(&slots);

    Ok(Ret(map))
}

// remove_slot(flyweight, symbol) - return copy of the same flyweight but with the slot of `symbol` name removed.
// No error is returned if the slot isn't present.
fn bf_remove_slot(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.config.flyweight_type {
        return Err(BfErr::ErrValue(E_PERM.msg("Flyweights not enabled")));
    }

    if bf_args.args.len() != 2 {
        return Err(BfErr::ErrValue(E_ARGS.with_msg(|| {
            format!(
                "remove_slot() takes 2 arguments, got {}",
                bf_args.args.len()
            )
        })));
    }

    let Variant::Flyweight(f) = bf_args.args[0].variant() else {
        return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
            format!(
                "remove_slot() expects a flyweight as the first argument, got {}",
                bf_args.args[0].type_code().to_literal()
            )
        })));
    };

    let Ok(s) = bf_args.args[1].as_symbol() else {
        return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
            format!(
                "remove_slot() expects a symbol as the second argument, got {}",
                bf_args.args[1].type_code().to_literal()
            )
        })));
    };

    let slots: Vec<_> = f
        .slots()
        .iter()
        .filter(|(k, _)| *k != s)
        .map(|(k, v)| (*k, v.clone()))
        .collect();

    let f = v_flyweight(f.delegate().clone(), &slots, f.contents().clone(), None);
    Ok(Ret(f))
}

/// add_slot(flyweight, key, value) - return copy of the same flyweight but with the slot of `key` name added or updated.
fn bf_add_slot(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if !bf_args.config.flyweight_type {
        return Err(BfErr::ErrValue(E_PERM.msg("Flyweights not enabled")));
    }

    if bf_args.args.len() != 3 {
        return Err(BfErr::ErrValue(E_ARGS.with_msg(|| {
            format!("add_slot() takes 3 arguments, got {}", bf_args.args.len())
        })));
    }

    let Variant::Flyweight(f) = bf_args.args[0].variant() else {
        return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
            format!(
                "add_slot() expects a flyweight as the first argument, got {}",
                bf_args.args[0].type_code().to_literal()
            )
        })));
    };

    let Ok(key) = bf_args.args[1].as_symbol() else {
        return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
            format!(
                "add_slot() expects a symbol as the second argument, got {}",
                bf_args.args[1].type_code().to_literal()
            )
        })));
    };

    let value = bf_args.args[2].clone();

    let mut slots: Vec<_> = f.slots().iter().map(|(k, v)| (*k, v.clone())).collect();

    // Add or update the slot
    if let Some(existing) = slots.iter_mut().find(|(k, _)| *k == key) {
        existing.1 = value;
    } else {
        slots.push((key, value));
    }
    let f = v_flyweight(f.delegate().clone(), &slots, f.contents().clone(), None);
    Ok(Ret(f))
}

pub(crate) fn register_bf_flyweights(builtins: &mut [Box<BuiltinFunction>]) {
    builtins[offset_for_builtin("xml_parse")] = Box::new(bf_xml_parse);
    builtins[offset_for_builtin("to_xml")] = Box::new(bf_to_xml);
    builtins[offset_for_builtin("slots")] = Box::new(bf_slots);
    builtins[offset_for_builtin("remove_slot")] = Box::new(bf_remove_slot);
    builtins[offset_for_builtin("add_slot")] = Box::new(bf_add_slot);
}
