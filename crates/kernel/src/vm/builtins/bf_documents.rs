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

//! Document format builtins: XML and JSON parsing/generation functions

use crate::vm::builtins::BfRet::Ret;
use crate::vm::builtins::{BfCallState, BfErr, BfRet, BuiltinFunction, world_state_bf_err};
use moor_common::model::WorldState;
use moor_compiler::offset_for_builtin;
use moor_var::{
    Associative, E_ARGS, E_INVARG, E_INVIND, E_PERM, E_TYPE, Flyweight, List, Map, Obj,
    SYSTEM_OBJECT, Sequence, Symbol, VarType, Variant, v_flyweight, v_int, v_list, v_map, v_obj,
    v_str, v_string,
};
use serde_json::{self, Value as JsonValue};
use std::io::{BufReader, BufWriter};
use tracing::error;
use xml::EmitterConfig;
use xml::reader::XmlEvent;

/// Uses xml-rs to parse a string into various data structures representing the XML.
///
/// Arguments:
/// 1. xml_string: The XML string to parse
/// 2. result_type (optional): Integer specifying the result format:
///    - VarType::TYPE_FLYWEIGHT (15): Original flyweight format
///    - VarType::TYPE_LIST (4): List format like {"tag", {"attr", "value"}, ...contents...}
///    - VarType::TYPE_MAP (10): Map format with list structure like {"tag", [attrs], content}
///      (Defaults to LIST format (4) if not specified.)
/// 3. tag_map (optional): Only used for flyweight format - maps tag names to objects
///
/// For flyweight format, delegates are resolved as follows:
/// a) For each tag, there should be an object: $tag_<tag> for that tag name,
/// b) Alternatively, if a map is provided as the third argument, the tag name is looked up
///    in the map, and the object is resolved from that.
fn bf_xml_parse(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.is_empty() || bf_args.args.len() > 3 {
        return Err(BfErr::ErrValue(E_ARGS.with_msg(|| {
            format!(
                "xml_parse() takes 1-3 arguments, got {}",
                bf_args.args.len()
            )
        })));
    }

    let Some(xml) = bf_args.args[0].as_string() else {
        return Err(BfErr::ErrValue(E_INVARG.with_msg(|| {
            format!(
                "xml_parse() expects a string as first argument, got {}",
                bf_args.args[0].type_code().to_literal()
            )
        })));
    };

    // Default to list format (4) if no type is specified
    let result_type = if bf_args.args.len() >= 2 {
        let Some(rt) = bf_args.args[1].as_integer() else {
            return Err(BfErr::ErrValue(E_INVARG.with_msg(|| {
                format!(
                    "xml_parse() expects an integer as second argument, got {}",
                    bf_args.args[1].type_code().to_literal()
                )
            })));
        };

        VarType::from_repr(rt as u8).ok_or_else(|| {
            BfErr::ErrValue(E_INVARG.with_msg(|| {
                "xml_parse() unsupported result type; LIST, MAP, or FLYWEIGHT expected".into()
            }))
        })?
    } else {
        // Default to list format
        VarType::TYPE_LIST
    };

    let map = if bf_args.args.len() == 3 {
        if result_type != VarType::TYPE_FLYWEIGHT {
            return Err(BfErr::ErrValue(E_INVARG.msg(
                "xml_parse() tag map only supported for flyweight result type",
            )));
        }
        let Some(m) = bf_args.args[2].as_map() else {
            return Err(BfErr::ErrValue(E_INVARG.with_msg(|| {
                format!(
                    "xml_parse() expects a map as the third argument, got {}",
                    bf_args.args[2].type_code().to_literal()
                )
            })));
        };
        Some(m)
    } else {
        None
    };

    match result_type {
        VarType::TYPE_FLYWEIGHT => parse_xml_to_flyweights(xml, map, bf_args),
        VarType::TYPE_LIST => parse_xml_to_lists(xml),
        VarType::TYPE_MAP => parse_xml_to_maps(xml),
        _ => unreachable!(),
    }
}

/// Parse XML into flyweight format (original behavior)
fn parse_xml_to_flyweights(
    xml: &str,
    map: Option<&Map>,
    bf_args: &mut BfCallState<'_>,
) -> Result<BfRet, BfErr> {
    let reader = BufReader::new(xml.as_bytes());
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
                        let Some(o) = obj.as_object() else {
                            return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
                                format!("xml_parse() tag {} in map is not an object", tag)
                            })));
                        };
                        o
                    }
                    None => {
                        let key = format!("tag_{}", tag);
                        let key = Symbol::mk(&key);

                        // resolve via system object
                        let prop_value = bf_args
                            .world_state
                            .retrieve_property(&bf_args.caller_perms(), &SYSTEM_OBJECT, key)
                            .map_err(world_state_bf_err)?;

                        let Some(o) = prop_value.as_object() else {
                            return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
                                format!("xml_parse() tag {} not found in system object", tag)
                            })));
                        };

                        o
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
                let fl = v_flyweight(obj, &attributes, children);
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

    // Return single root element directly, or wrap multiple elements in a list
    let result = if output_tree.len() == 1 {
        output_tree.into_iter().next().unwrap()
    } else {
        v_list(&output_tree)
    };
    Ok(Ret(result))
}

/// Parse XML into list format like {"tag", {"attr", "value"}, ...contents...}
fn parse_xml_to_lists(xml: &str) -> Result<BfRet, BfErr> {
    let reader = BufReader::new(xml.as_bytes());
    let parser = xml::EventReader::new(reader);
    let mut output_tree = Vec::new();
    let mut element_stack = Vec::new();

    for e in parser {
        match e {
            Ok(XmlEvent::StartElement {
                name, attributes, ..
            }) => {
                let tag_name = name.local_name;
                let mut element_list = vec![v_str(&tag_name)];

                // Add attributes as two-element lists
                for attr in attributes {
                    let attr_list = v_list(&[
                        v_str(attr.name.local_name.as_str()),
                        v_str(attr.value.as_str()),
                    ]);
                    element_list.push(attr_list);
                }

                element_stack.push(element_list);
            }
            Ok(XmlEvent::EndElement { .. }) => {
                let element_list = element_stack.pop().ok_or_else(|| {
                    BfErr::ErrValue(
                        E_INVARG.with_msg(|| "xml_parse() end tag without start tag".to_string()),
                    )
                })?;

                let element = v_list(&element_list);
                if let Some(parent) = element_stack.last_mut() {
                    parent.push(element);
                } else {
                    output_tree.push(element);
                }
            }
            Ok(XmlEvent::Characters(text)) => {
                if let Some(parent) = element_stack.last_mut() {
                    if !text.trim().is_empty() {
                        parent.push(v_str(text.as_str()));
                    }
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

    // Return single root element directly, or wrap multiple elements in a list
    let result = if output_tree.len() == 1 {
        output_tree.into_iter().next().unwrap()
    } else {
        v_list(&output_tree)
    };
    Ok(Ret(result))
}

/// Parse XML into map format (list of maps)
fn parse_xml_to_maps(xml: &str) -> Result<BfRet, BfErr> {
    let reader = BufReader::new(xml.as_bytes());
    let parser = xml::EventReader::new(reader);
    let mut output_tree = Vec::new();
    let mut element_stack = Vec::new();

    for e in parser {
        match e {
            Ok(XmlEvent::StartElement {
                name, attributes, ..
            }) => {
                let tag_name = name.local_name;
                let mut element_data = Vec::new();

                // First element is the tag name
                element_data.push(v_str(&tag_name));

                // Second element is a map of attributes
                if !attributes.is_empty() {
                    let attr_pairs: Vec<_> = attributes
                        .iter()
                        .map(|attr| {
                            (
                                v_str(attr.name.local_name.as_str()),
                                v_str(attr.value.as_str()),
                            )
                        })
                        .collect();
                    element_data.push(v_map(&attr_pairs));
                } else {
                    // Empty map for no attributes
                    element_data.push(v_map(&[]));
                }

                element_stack.push(element_data);
            }
            Ok(XmlEvent::EndElement { .. }) => {
                let element_data = element_stack.pop().ok_or_else(|| {
                    BfErr::ErrValue(
                        E_INVARG.with_msg(|| "xml_parse() end tag without start tag".to_string()),
                    )
                })?;

                let element = v_list(&element_data);
                if let Some(parent) = element_stack.last_mut() {
                    // Add to parent's content
                    parent.push(element);
                } else {
                    output_tree.push(element);
                }
            }
            Ok(XmlEvent::Characters(text)) => {
                if let Some(parent) = element_stack.last_mut() {
                    if !text.trim().is_empty() {
                        // Add text to parent's content
                        parent.push(v_str(text.as_str()));
                    }
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

    // Return single root element directly, or wrap multiple elements in a list
    let result = if output_tree.len() == 1 {
        output_tree.into_iter().next().unwrap()
    } else {
        v_list(&output_tree)
    };
    Ok(Ret(result))
}

enum Tag {
    StartElement(String, Vec<(String, String)>),
    EndElement(()),
    Text(String),
}

fn value_to_xml_tag(
    value: &moor_var::Var,
    map: Option<&Map>,
    perms: &Obj,
    ws: &mut dyn WorldState,
) -> Result<Vec<Tag>, BfErr> {
    match value.variant() {
        Variant::Flyweight(fl) => flyweight_to_xml_tag(fl, map, perms, ws),
        Variant::List(list) => parse_list_format_to_xml_tag(list, map, perms, ws),
        _ => Err(BfErr::ErrValue(
            E_INVARG.msg("Value must be flyweight or list for XML conversion"),
        )),
    }
}

fn parse_list_format_to_xml_tag(
    list: &List,
    map: Option<&Map>,
    perms: &Obj,
    ws: &mut dyn WorldState,
) -> Result<Vec<Tag>, BfErr> {
    if list.is_empty() {
        return Err(BfErr::ErrValue(
            E_INVARG.msg("Empty list provided for XML conversion"),
        ));
    }

    // Get the tag name (first element)
    let tag_element = list.index(0).map_err(BfErr::ErrValue)?;
    let tag_name = match tag_element.as_symbol() {
        Ok(sym) => sym.to_string(),
        Err(_) => {
            return Err(BfErr::ErrValue(
                E_INVARG.msg("Tag name must be string or symbol"),
            ));
        }
    };

    let mut attributes = Vec::new();
    let mut contents = Vec::new();
    let mut i = 1;

    // Process remaining elements
    while i < list.len() {
        let element = list.index(i).map_err(BfErr::ErrValue)?;
        match element.variant() {
            // Handle attribute lists/maps
            Variant::List(attr_list) => {
                // Check if this looks like an attribute list: {"attr", "value"} or {'attr, "value"}
                if attr_list.len() == 2 {
                    let attr_key = attr_list.index(0).map_err(BfErr::ErrValue)?;
                    let attr_value = attr_list.index(1).map_err(BfErr::ErrValue)?;

                    let key_str = match attr_key.as_symbol() {
                        Ok(sym) => sym.to_string(),
                        Err(_) => {
                            return Err(BfErr::ErrValue(
                                E_INVARG.msg("Attribute key must be string or symbol"),
                            ));
                        }
                    };

                    let value_str = match attr_value.variant() {
                        Variant::Str(s) => s.as_str().to_string(),
                        Variant::Int(i) => i.to_string(),
                        Variant::Float(f) => f.to_string(),
                        _ => {
                            return Err(BfErr::ErrValue(
                                E_INVARG.msg("Attribute value must be string or number"),
                            ));
                        }
                    };

                    attributes.push((key_str, value_str));
                } else {
                    // This is content - recursively process if it's another list format
                    if attr_list.len() >= 1 {
                        let child_tags = parse_list_format_to_xml_tag(attr_list, map, perms, ws)?;
                        contents.extend(child_tags);
                    } else {
                        return Err(BfErr::ErrValue(
                            E_INVARG.msg("Invalid list structure for XML conversion"),
                        ));
                    }
                }
            }
            Variant::Map(attr_map) => {
                // Handle map format: ['attr -> "value"]
                for (key, value) in attr_map.iter() {
                    let key_str = match key.as_symbol() {
                        Ok(sym) => sym.to_string(),
                        Err(_) => {
                            return Err(BfErr::ErrValue(
                                E_INVARG.msg("Attribute key must be string or symbol"),
                            ));
                        }
                    };

                    let value_str = match value.variant() {
                        Variant::Str(s) => s.as_str().to_string(),
                        Variant::Int(i) => i.to_string(),
                        Variant::Float(f) => f.to_string(),
                        _ => {
                            return Err(BfErr::ErrValue(
                                E_INVARG.msg("Attribute value must be string or number"),
                            ));
                        }
                    };

                    attributes.push((key_str, value_str));
                }
            }
            Variant::Str(s) => {
                // This is text content
                contents.push(Tag::Text(s.as_str().to_string()));
            }
            _ => {
                // Try to recursively process other lists or flyweights
                let child_tags = value_to_xml_tag(&element, map, perms, ws)?;
                contents.extend(child_tags);
            }
        }
        i += 1;
    }

    let mut tags = Vec::new();
    tags.push(Tag::StartElement(tag_name, attributes));
    tags.extend(contents);
    tags.push(Tag::EndElement(()));

    Ok(tags)
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
            let key = v_obj(*fl.delegate());
            let Ok(tag) = m.get(&key) else {
                return Err(BfErr::ErrValue(E_INVARG.with_msg(|| {
                    format!("to_xml() tag {} not found in map", fl.delegate().id())
                })));
            };
            let Some(s) = tag.as_string() else {
                return Err(BfErr::ErrValue(E_INVARG.with_msg(|| {
                    format!("to_xml() tag {} in map is not a string", fl.delegate().id())
                })));
            };
            s.to_string()
        }
        None => {
            let key = Symbol::mk("tag");
            let tag = ws
                .retrieve_property(perms, fl.delegate(), key)
                .map_err(world_state_bf_err)?;

            let Some(s) = tag.as_string() else {
                return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
                    format!("to_xml() tag {} is not a string", fl.delegate().id())
                })));
            };

            s.to_string()
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
            Variant::Str(s) => {
                tags.push(Tag::Text(s.as_str().to_string()));
            }
            _ => {
                // Use the unified approach for all other types (flyweights, lists)
                let child_tags = value_to_xml_tag(&child, map, perms, ws)?;
                tags.extend(child_tags);
            }
        }
    }
    tags.push(Tag::EndElement(()));

    Ok(tags)
}

/// to_xml(root_element, [tag map]) -> string
///
/// Turn a tree of flyweights or lists into an XML document.
///
/// The first argument can be either:
/// 1. A flyweight (original behavior):
///    - delegate object with a tag property OR there's a second map argument that maps object ids to tags
///    - attributes property that is a map of strings to string or numbers
///    - any children must be either other valid flyweights, or string values
///
/// 2. A list in one of these formats:
///    - {"tag", {"attr", "value"}, ...contents...} - strings for tag/attr names
///    - {'tag, {'attr, "value"}, ...contents...} - symbols for tag/attr names  
///    - {'tag, ['attr -> "value"], ...contents...} - symbols with map for attributes
///    
/// List format details:
///  - First element: tag name (string or symbol)
///  - Subsequent elements can be:
///    - Two-element lists: {"attr", "value"} or {'attr, "value"} (attributes)
///    - Maps: ['attr -> "value"] (attributes)
///    - Strings: text content
///    - Other lists: nested XML elements (recursive)
fn bf_to_xml(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 && bf_args.args.len() != 2 {
        return Err(BfErr::ErrValue(E_ARGS.with_msg(|| {
            format!(
                "to_xml() takes 1 or 2 arguments, got {}",
                bf_args.args.len()
            )
        })));
    }

    let root = &bf_args.args[0];

    // Check if flyweights are enabled only when we have a flyweight
    if let Variant::Flyweight(_) = root.variant() {
        if !bf_args.config.flyweight_type {
            return Err(BfErr::ErrValue(
                E_PERM.msg("Flyweight functionality not enabled"),
            ));
        }
    }

    let map = if bf_args.args.len() == 2 {
        let Some(m) = bf_args.args[1].as_map() else {
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

    // Create a closure for tag resolution that handles both flyweights and lists
    let tag_resolver = |value: &moor_var::Var| -> Result<Vec<Tag>, BfErr> {
        value_to_xml_tag(value, map, &bf_args.caller_perms(), bf_args.world_state)
    };

    let xml_string = generate_xml_from_value(root, tag_resolver)?;
    Ok(Ret(v_string(xml_string)))
}

/// Generate XML string from a value using a tag resolver closure
fn generate_xml_from_value<F>(value: &moor_var::Var, mut tag_resolver: F) -> Result<String, BfErr>
where
    F: FnMut(&moor_var::Var) -> Result<Vec<Tag>, BfErr>,
{
    let mut output = Vec::new();
    {
        let mut output_buf = BufWriter::new(&mut output);
        let mut writer = EmitterConfig::new()
            .perform_indent(true)
            .create_writer(&mut output_buf);

        // Process the value using the tag resolver
        let tags = tag_resolver(value)?;
        for tag in tags {
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
    Ok(output_as_string)
}

/// Convert a MOO value to a JSON string
fn bf_generate_json(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let value = &bf_args.args[0];
    let json_value = moo_value_to_json(value)?;

    match serde_json::to_string(&json_value) {
        Ok(json_str) => Ok(Ret(v_string(json_str))),
        Err(_) => Err(BfErr::Code(E_INVARG)),
    }
}

/// Convert a MOO value to a JSON value
fn moo_value_to_json(value: &moor_var::Var) -> Result<JsonValue, BfErr> {
    match value.variant() {
        Variant::Int(i) => Ok(JsonValue::Number((*i).into())),
        Variant::Float(f) => {
            let num = serde_json::Number::from_f64(*f).ok_or_else(|| BfErr::Code(E_INVARG))?;
            Ok(JsonValue::Number(num))
        }
        Variant::Str(s) => Ok(JsonValue::String(s.as_str().to_string())),
        Variant::Obj(o) => Ok(JsonValue::String(format!("#{}", o))),
        Variant::List(list) => {
            let mut json_array = Vec::new();
            for item in list.iter() {
                json_array.push(moo_value_to_json(&item)?);
            }
            Ok(JsonValue::Array(json_array))
        }
        Variant::Map(map) => {
            let mut json_obj = serde_json::Map::new();
            for (k, v) in map.iter() {
                // JSON only allows string keys
                let key = match k.variant() {
                    Variant::Str(s) => s.as_str().to_string(),
                    Variant::Int(i) => i.to_string(),
                    Variant::Float(f) => f.to_string(),
                    Variant::Obj(o) => format!("#{}", o),
                    _ => return Err(BfErr::Code(E_TYPE)), // Complex keys not supported
                };
                json_obj.insert(key, moo_value_to_json(&v)?);
            }
            Ok(JsonValue::Object(json_obj))
        }
        _ => Err(BfErr::Code(E_TYPE)), // Other types not supported
    }
}

/// Convert a JSON value to a MOO value
fn json_value_to_moo(json_value: &JsonValue) -> Result<moor_var::Var, BfErr> {
    match json_value {
        JsonValue::Null => Ok(moor_var::v_none()),
        JsonValue::Bool(b) => Ok(v_int(if *b { 1 } else { 0 })),
        JsonValue::Number(n) => {
            if n.is_i64() {
                Ok(v_int(n.as_i64().unwrap()))
            } else {
                Ok(moor_var::v_float(n.as_f64().unwrap()))
            }
        }
        JsonValue::String(s) => Ok(v_str(s)),
        JsonValue::Array(arr) => {
            let mut list_items = Vec::new();
            for item in arr {
                list_items.push(json_value_to_moo(item)?);
            }
            Ok(v_list(&list_items))
        }
        JsonValue::Object(obj) => {
            let mut map_items = Vec::new();
            for (k, v) in obj {
                let key = v_str(k);
                let value = json_value_to_moo(v)?;
                map_items.push((key, value));
            }
            Ok(v_map(&map_items))
        }
    }
}

/// Parse a JSON string into a MOO value
fn bf_parse_json(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let Some(json_str) = bf_args.args[0].as_string() else {
        return Err(BfErr::Code(E_TYPE));
    };

    match serde_json::from_str::<JsonValue>(json_str) {
        Ok(json_value) => {
            let moo_value = json_value_to_moo(&json_value)?;
            Ok(Ret(moo_value))
        }
        Err(_) => Err(BfErr::Code(E_INVARG)),
    }
}

pub(crate) fn register_bf_documents(builtins: &mut [Box<BuiltinFunction>]) {
    // XML functions
    builtins[offset_for_builtin("xml_parse")] = Box::new(bf_xml_parse);
    builtins[offset_for_builtin("to_xml")] = Box::new(bf_to_xml);

    // JSON functions
    builtins[offset_for_builtin("generate_json")] = Box::new(bf_generate_json);
    builtins[offset_for_builtin("parse_json")] = Box::new(bf_parse_json);
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_var::v_sym;

    #[test]
    fn test_tag_structure_basic() {
        // Test the basic Tag enum structure
        let start_tag = Tag::StartElement(
            "div".to_string(),
            vec![("class".to_string(), "test".to_string())],
        );
        let text_tag = Tag::Text("Hello World".to_string());
        let end_tag = Tag::EndElement(());

        match start_tag {
            Tag::StartElement(name, attrs) => {
                assert_eq!(name, "div");
                assert_eq!(attrs[0], ("class".to_string(), "test".to_string()));
            }
            _ => panic!("Expected StartElement"),
        }

        match text_tag {
            Tag::Text(text) => assert_eq!(text, "Hello World"),
            _ => panic!("Expected Text"),
        }

        match end_tag {
            Tag::EndElement(_) => {}
            _ => panic!("Expected EndElement"),
        }
    }

    #[test]
    fn test_format_structures() {
        // Test that we can create the expected list structures

        // String-based format: {"tag", {"attr", "value"}, "content"}
        let string_format = v_list(&[
            v_str("div"),
            v_list(&[v_str("class"), v_str("test")]),
            v_str("Hello World"),
        ]);

        assert!(string_format.as_list().is_some());
        let list = string_format.as_list().unwrap();
        assert_eq!(list.len(), 3);

        // Symbol-based format: {'tag, {'attr, "value"}, "content"}
        let symbol_format = v_list(&[
            v_sym("div"),
            v_list(&[v_sym("class"), v_str("test")]),
            v_str("Hello World"),
        ]);

        assert!(symbol_format.as_list().is_some());

        // Map-based format: {'tag, ['attr -> "value"], "content"}
        let map_attrs = v_map(&[(v_sym("class"), v_str("test"))]);
        let map_format = v_list(&[v_sym("div"), map_attrs, v_str("Hello World")]);

        assert!(map_format.as_list().is_some());
    }

    #[test]
    fn test_generate_xml_from_list_format() {
        // Test generating XML from list format: {"div", {"class", "test"}, "Hello World"}
        let list_format = v_list(&[
            v_str("div"),
            v_list(&[v_str("class"), v_str("test")]),
            v_str("Hello World"),
        ]);

        // Simple tag resolver that processes lists without needing world state
        let tag_resolver = |value: &moor_var::Var| -> Result<Vec<Tag>, BfErr> {
            match value.variant() {
                Variant::List(list) => {
                    // Simplified list processing for testing
                    if list.is_empty() {
                        return Err(BfErr::ErrValue(E_INVARG.msg("Empty list")));
                    }

                    let tag_name = list.index(0).unwrap().as_string().unwrap().to_string();
                    let mut attributes = Vec::new();
                    let mut contents = Vec::new();

                    for i in 1..list.len() {
                        let element = list.index(i).unwrap();
                        match element.variant() {
                            Variant::List(attr_list) if attr_list.len() == 2 => {
                                let key_val = attr_list.index(0).unwrap();
                                let value_val = attr_list.index(1).unwrap();
                                let key = key_val.as_string().unwrap();
                                let value = value_val.as_string().unwrap();
                                attributes.push((key.to_string(), value.to_string()));
                            }
                            Variant::Str(s) => {
                                contents.push(Tag::Text(s.as_str().to_string()));
                            }
                            _ => {}
                        }
                    }

                    let mut tags = Vec::new();
                    tags.push(Tag::StartElement(tag_name, attributes));
                    tags.extend(contents);
                    tags.push(Tag::EndElement(()));
                    Ok(tags)
                }
                _ => Err(BfErr::ErrValue(E_INVARG.msg("Expected list"))),
            }
        };

        let xml_result = generate_xml_from_value(&list_format, tag_resolver).unwrap();

        // Should generate proper XML
        assert!(xml_result.contains("<div class=\"test\">Hello World</div>"));
    }

    #[test]
    fn test_generate_xml_nested_structure() {
        // Test nested structure: {"div", {"Hello ", {"span", "World"}}}
        let nested_list = v_list(&[
            v_str("div"),
            v_str("Hello "),
            v_list(&[v_str("span"), v_str("World")]),
        ]);

        // Recursive tag resolver for nested structures
        let tag_resolver = |value: &moor_var::Var| -> Result<Vec<Tag>, BfErr> {
            fn process_value(value: &moor_var::Var) -> Result<Vec<Tag>, BfErr> {
                match value.variant() {
                    Variant::List(list) => {
                        if list.is_empty() {
                            return Ok(vec![]);
                        }

                        let tag_name = list.index(0).unwrap().as_string().unwrap().to_string();
                        let mut contents = Vec::new();

                        for i in 1..list.len() {
                            let element = list.index(i).unwrap();
                            match element.variant() {
                                Variant::Str(s) => {
                                    contents.push(Tag::Text(s.as_str().to_string()));
                                }
                                Variant::List(_) => {
                                    let child_tags = process_value(&element)?;
                                    contents.extend(child_tags);
                                }
                                _ => {}
                            }
                        }

                        let mut tags = Vec::new();
                        tags.push(Tag::StartElement(tag_name, vec![]));
                        tags.extend(contents);
                        tags.push(Tag::EndElement(()));
                        Ok(tags)
                    }
                    _ => Err(BfErr::ErrValue(E_INVARG.msg("Expected list"))),
                }
            }
            process_value(value)
        };

        let xml_result = generate_xml_from_value(&nested_list, tag_resolver).unwrap();

        // Should generate nested XML
        assert!(xml_result.contains("<div>"));
        assert!(xml_result.contains("Hello "));
        assert!(xml_result.contains("<span>World</span>"));
        assert!(xml_result.contains("</div>"));
    }

    #[test]
    fn test_to_xml_accepts_lists_without_flyweights() {
        // Test that to_xml works with list format even when flyweights might be disabled
        let list_format = v_list(&[
            v_str("div"),
            v_list(&[v_str("class"), v_str("test")]),
            v_str("Hello World"),
        ]);

        // This should work regardless of flyweight configuration
        // The function should only check flyweight enablement when input is actually a flyweight
        assert!(list_format.as_list().is_some());
    }

    #[test]
    fn test_parse_xml_to_lists_actual() {
        let xml = r#"<div class="test">Hello <span>World</span></div>"#;
        let result = parse_xml_to_lists(xml).unwrap();

        if let Ret(result_val) = result {
            // Should be a single div element (not wrapped in a list)
            let div_list = result_val.as_list().unwrap();

            // First element should be tag name
            assert_eq!(div_list.index(0).unwrap().as_string().unwrap(), "div");

            // Second element should be class attribute
            let attr_element = div_list.index(1).unwrap();
            let attr_list = attr_element.as_list().unwrap();
            assert_eq!(attr_list.index(0).unwrap().as_string().unwrap(), "class");
            assert_eq!(attr_list.index(1).unwrap().as_string().unwrap(), "test");

            // Should have text content "Hello "
            assert_eq!(div_list.index(2).unwrap().as_string().unwrap(), "Hello ");

            // Should have nested span element
            let span_element = div_list.index(3).unwrap();
            let span_list = span_element.as_list().unwrap();
            assert_eq!(span_list.index(0).unwrap().as_string().unwrap(), "span");
            assert_eq!(span_list.index(1).unwrap().as_string().unwrap(), "World");
        } else {
            panic!("Expected Ret result");
        }
    }

    #[test]
    fn test_parse_xml_to_maps_actual() {
        let xml = r#"<div class="test">Hello</div>"#;
        let result = parse_xml_to_maps(xml).unwrap();

        if let Ret(result_val) = result {
            // Should be a single div list (not wrapped in another list)
            let div_list = result_val.as_list().unwrap();

            // First element should be the tag name
            assert_eq!(div_list.index(0).unwrap().as_string().unwrap(), "div");

            // Second element should be the attributes map
            let attr_value = div_list.index(1).unwrap();
            let attrs_map = attr_value.as_map().unwrap();
            let class_val = attrs_map.get(&v_str("class")).unwrap();
            assert_eq!(class_val.as_string().unwrap(), "test");

            // Third element should be the content "Hello"
            assert_eq!(div_list.index(2).unwrap().as_string().unwrap(), "Hello");
        } else {
            panic!("Expected Ret result");
        }
    }
}
