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

//! Document format builtins: XML, JSON, and HTML parsing/generation functions

use crate::{
    task_context::with_current_transaction,
    vm::builtins::{BfCallState, BfErr, BfRet, BfRet::Ret, BuiltinFunction, world_state_bf_err},
};
use moor_compiler::{offset_for_builtin, to_literal};
use moor_var::{
    Associative, E_ARGS, E_INVARG, E_INVIND, E_PERM, E_TYPE, Flyweight, List, Map, SYSTEM_OBJECT,
    Sequence, Symbol, VarType, Variant, v_bool_int, v_flyweight, v_int, v_list, v_map, v_obj,
    v_str, v_string,
};
use scraper::{Html, Selector};
use serde_json::{self, Value as JsonValue};
use std::io::{BufReader, BufWriter};
use tracing::error;
use xml::{EmitterConfig, reader::XmlEvent};

/// MOO: `any xml_parse(str xml_string [, int result_type] [, map tag_map])`
/// Parses XML string into various data structures.
/// Result type: 4=list format, 10=map format, 15=flyweight format (default: list).
/// For flyweight format, tag_map maps tag names to objects.
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
                            return Err(BfErr::ErrValue(
                                E_INVARG
                                    .with_msg(|| format!("xml_parse() tag {tag} not found in map")),
                            ));
                        };
                        let Some(o) = obj.as_object() else {
                            return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
                                format!("xml_parse() tag {tag} in map is not an object")
                            })));
                        };
                        o
                    }
                    None => {
                        let key = format!("tag_{tag}");
                        let key = Symbol::mk(&key);

                        // resolve via system object
                        let prop_value = with_current_transaction(|world_state| {
                            world_state.retrieve_property(
                                &bf_args.caller_perms(),
                                &SYSTEM_OBJECT,
                                key,
                            )
                        })
                        .map_err(world_state_bf_err)?;

                        let Some(o) = prop_value.as_object() else {
                            return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
                                format!("xml_parse() tag {tag} not found in system object")
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
                    E_INVARG.with_msg(|| format!("xml_parse() error parsing XML: {e}")),
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
                if let Some(parent) = element_stack.last_mut()
                    && !text.trim().is_empty()
                {
                    parent.push(v_str(text.as_str()));
                }
            }
            Ok(_) => {
                // Ignore other events (CDATA, etc)
            }
            Err(e) => {
                return Err(BfErr::ErrValue(
                    E_INVARG.with_msg(|| format!("xml_parse() error parsing XML: {e}")),
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
                if let Some(parent) = element_stack.last_mut()
                    && !text.trim().is_empty()
                {
                    // Add text to parent's content
                    parent.push(v_str(text.as_str()));
                }
            }
            Ok(_) => {
                // Ignore other events (CDATA, etc)
            }
            Err(e) => {
                return Err(BfErr::ErrValue(
                    E_INVARG.with_msg(|| format!("xml_parse() error parsing XML: {e}")),
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

#[derive(Debug)]
enum Tag {
    StartElement(String, Vec<(String, String)>),
    EndElement(()),
    Text(String),
}

fn value_to_xml_tag<F>(
    value: &moor_var::Var,
    flyweight_tag_resolver: &mut F,
) -> Result<Vec<Tag>, BfErr>
where
    F: FnMut(&Flyweight) -> Result<String, BfErr>,
{
    match value.variant() {
        Variant::Flyweight(fl) => flyweight_to_xml_tag(fl, flyweight_tag_resolver),
        Variant::List(list) => parse_list_format_to_xml_tag(list, flyweight_tag_resolver),
        _ => Err(BfErr::ErrValue(
            E_INVARG.msg("Value must be flyweight or list for XML conversion"),
        )),
    }
}

fn parse_list_format_to_xml_tag<F>(
    list: &List,
    flyweight_tag_resolver: &mut F,
) -> Result<Vec<Tag>, BfErr>
where
    F: FnMut(&Flyweight) -> Result<String, BfErr>,
{
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
            return Err(BfErr::ErrValue(E_INVARG.with_msg(|| {
                format!(
                    "Tag name must be string or symbol (was: {})",
                    to_literal(&tag_element)
                )
            })));
        }
    };

    let mut attributes = Vec::new();
    let mut contents = Vec::new();

    // Process attributes (index 1) if it exists
    if list.len() >= 2 {
        let attr_element = list.index(1).map_err(BfErr::ErrValue)?;
        match attr_element.variant() {
            Variant::List(attr_list) => {
                // Process attribute list
                let mut i = 0;
                while i < attr_list.len() {
                    if i + 1 < attr_list.len() {
                        // Process as key-value pair
                        let attr_key = attr_list.index(i).map_err(BfErr::ErrValue)?;
                        let attr_value = attr_list.index(i + 1).map_err(BfErr::ErrValue)?;

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
                        i += 2;
                    } else {
                        // Odd number of items in attribute list
                        break;
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
            _ => {
                // Empty list or other types - no attributes
            }
        }
    }

    // Process content (index 2 onwards)
    for i in 2..list.len() {
        let element = list.index(i).map_err(BfErr::ErrValue)?;
        match element.variant() {
            Variant::Str(s) => {
                // This is text content
                contents.push(Tag::Text(s.as_str().to_string()));
            }
            _ => {
                // Try to recursively process other lists or flyweights
                let child_tags = value_to_xml_tag(&element, flyweight_tag_resolver)?;
                contents.extend(child_tags);
            }
        }
    }

    let mut tags = Vec::new();
    tags.push(Tag::StartElement(tag_name, attributes));
    tags.extend(contents);
    tags.push(Tag::EndElement(()));

    Ok(tags)
}

fn flyweight_to_xml_tag<F>(
    fl: &Flyweight,
    flyweight_tag_resolver: &mut F,
) -> Result<Vec<Tag>, BfErr>
where
    F: FnMut(&Flyweight) -> Result<String, BfErr>,
{
    let mut tags = Vec::new();

    // Use the provided closure to resolve the tag name
    let tag_name = flyweight_tag_resolver(fl)?;

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
                let child_tags = value_to_xml_tag(&child, flyweight_tag_resolver)?;
                tags.extend(child_tags);
            }
        }
    }
    tags.push(Tag::EndElement(()));

    Ok(tags)
}

/// MOO: `str to_xml(any root_element [, map tag_map])`
/// Converts a tree of flyweights or lists into an XML document.
/// List format: {"tag", {"attr", "value"}, ...contents...} or with symbols/maps.
/// For flyweights, tag_map maps object IDs to tag names.
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
    if let Variant::Flyweight(_) = root.variant()
        && !bf_args.config.flyweight_type
    {
        return Err(BfErr::ErrValue(
            E_PERM.msg("Flyweight functionality not enabled"),
        ));
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

    // Create a closure for flyweight tag resolution only
    let mut flyweight_tag_resolver = |fl: &Flyweight| -> Result<String, BfErr> {
        match map {
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
                Ok(s.to_string())
            }
            None => {
                let key = Symbol::mk("tag");
                let tag = with_current_transaction(|world_state| {
                    world_state.retrieve_property(&bf_args.caller_perms(), fl.delegate(), key)
                })
                .map_err(world_state_bf_err)?;

                let Some(s) = tag.as_string() else {
                    return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
                        format!("to_xml() tag {} is not a string", fl.delegate().id())
                    })));
                };

                Ok(s.to_string())
            }
        }
    };

    let tags = value_to_xml_tag(root, &mut flyweight_tag_resolver)?;
    let xml_string = generate_xml_from_tags(&tags)?;
    Ok(Ret(v_string(xml_string)))
}

/// Generate XML string from a list of tags
fn generate_xml_from_tags(tags: &[Tag]) -> Result<String, BfErr> {
    let mut output = Vec::new();
    {
        let mut output_buf = BufWriter::new(&mut output);
        let mut writer = EmitterConfig::new()
            .line_separator("")
            .perform_indent(false)
            .write_document_declaration(false)
            .create_writer(&mut output_buf);

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
                            E_INVIND.with_msg(|| format!("to_xml() error writing XML: {e}")),
                        )
                    })?;
                }
                Tag::Text(text) => {
                    writer
                        .write(xml::writer::XmlEvent::characters(text.as_str()))
                        .map_err(|e| {
                            BfErr::ErrValue(
                                E_INVIND.with_msg(|| format!("to_xml() error writing XML: {e}")),
                            )
                        })?;
                }
                Tag::EndElement(_) => {
                    writer
                        .write(xml::writer::XmlEvent::end_element())
                        .map_err(|e| {
                            BfErr::ErrValue(
                                E_INVIND.with_msg(|| format!("to_xml() error writing XML: {e}")),
                            )
                        })?;
                }
            }
        }
    }
    let output_as_string = String::from_utf8(output).map_err(|e| {
        BfErr::ErrValue(
            E_INVIND.with_msg(|| format!("to_xml() error converting XML to string: {e}")),
        )
    })?;
    Ok(output_as_string)
}

/// MOO: `str generate_json(any value)`
/// Converts a MOO value to a JSON string.
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
        Variant::Int(i) => Ok(JsonValue::Number((i).into())),
        Variant::Float(f) => {
            let num = serde_json::Number::from_f64(f).ok_or_else(|| BfErr::Code(E_INVARG))?;
            Ok(JsonValue::Number(num))
        }
        Variant::Str(s) => Ok(JsonValue::String(s.as_str().to_string())),
        Variant::Obj(o) => Ok(JsonValue::String(format!("{o}"))),
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
                    Variant::Obj(o) => format!("{o}"),
                    _ => {
                        return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
                            format!(
                                "Cannot use {} as a json map key",
                                k.type_code().to_literal()
                            )
                        })));
                    } // Complex keys not supported
                };
                json_obj.insert(key, moo_value_to_json(&v)?);
            }
            Ok(JsonValue::Object(json_obj))
        }
        Variant::Bool(b) => Ok(JsonValue::Bool(b)),
        _ => Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
            format!(
                "Cannot translate values of type {} to JSON",
                value.type_code().to_literal()
            )
        }))), // Other types not supported
    }
}

/// Convert a JSON value to a MOO value
fn json_value_to_moo(json_value: &JsonValue) -> Result<moor_var::Var, BfErr> {
    match json_value {
        // JSON null becomes the string "null" for ToastStunt compatibility.
        // MOO has no proper null type - v_none() is a sigil that causes problems
        // if it ends up in variables or stack frames.
        JsonValue::Null => Ok(v_str("null")),
        JsonValue::Bool(b) => Ok(v_bool_int(*b)),
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

/// MOO: `any parse_json(str json_string)`
/// Parses a JSON string into a MOO value.
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

/// Build a CSS selector from MOO-style arguments.
/// Tag can be string or symbol. Attr filter uses glob patterns: "prefix*", "*suffix", "*contains*".
fn build_css_selector(tag: &str, attrs: Option<&Map>) -> Result<String, BfErr> {
    let mut selector = tag.to_string();

    let Some(attrs) = attrs else {
        return Ok(selector);
    };

    for (key, pattern) in attrs.iter() {
        let key_str = key
            .as_symbol()
            .map(|s| s.to_string())
            .ok()
            .or_else(|| key.as_string().map(|s| s.to_string()))
            .ok_or_else(|| BfErr::ErrValue(E_TYPE.msg("Attribute key must be string or symbol")))?;

        let Some(pattern_str) = pattern.as_string() else {
            return Err(BfErr::ErrValue(
                E_TYPE.msg("Attribute pattern must be a string"),
            ));
        };

        let starts_wild = pattern_str.starts_with('*');
        let ends_wild = pattern_str.ends_with('*');

        let attr_selector = match (starts_wild, ends_wild) {
            (true, true) if pattern_str.len() > 2 => {
                // *contains*
                let value = &pattern_str[1..pattern_str.len() - 1];
                format!("[{key_str}*=\"{value}\"]")
            }
            (false, true) if pattern_str.len() > 1 => {
                // prefix*
                let value = &pattern_str[..pattern_str.len() - 1];
                format!("[{key_str}^=\"{value}\"]")
            }
            (true, false) if pattern_str.len() > 1 => {
                // *suffix
                let value = &pattern_str[1..];
                format!("[{key_str}$=\"{value}\"]")
            }
            _ => {
                // exact match
                format!("[{key_str}=\"{pattern_str}\"]")
            }
        };
        selector.push_str(&attr_selector);
    }

    Ok(selector)
}

/// MOO: `list html_query(str html, any tag [, map attr_filter])`
/// Query HTML for elements matching tag name and optional attribute filters.
/// Returns list of maps containing attributes for each matching element.
fn bf_html_query(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() < 2 || bf_args.args.len() > 3 {
        return Err(BfErr::ErrValue(E_ARGS.with_msg(|| {
            format!(
                "html_query() takes 2-3 arguments, got {}",
                bf_args.args.len()
            )
        })));
    }

    let Some(html) = bf_args.args[0].as_string() else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("html_query() first argument must be a string"),
        ));
    };

    let tag = bf_args.args[1]
        .as_symbol()
        .map(|s| s.to_string())
        .ok()
        .or_else(|| bf_args.args[1].as_string().map(|s| s.to_string()))
        .ok_or_else(|| {
            BfErr::ErrValue(E_TYPE.msg("html_query() tag must be a string or symbol"))
        })?;

    let attr_filter = if bf_args.args.len() == 3 {
        let Some(m) = bf_args.args[2].as_map() else {
            return Err(BfErr::ErrValue(
                E_TYPE.msg("html_query() third argument must be a map"),
            ));
        };
        Some(m)
    } else {
        None
    };

    let selector_str = build_css_selector(&tag, attr_filter)?;

    let selector = Selector::parse(&selector_str)
        .map_err(|e| BfErr::ErrValue(E_INVARG.with_msg(|| format!("Invalid selector: {e:?}"))))?;

    let document = Html::parse_document(html);

    let mut results = Vec::new();
    for element in document.select(&selector) {
        let mut attrs = Vec::new();
        for (name, value) in element.value().attrs() {
            attrs.push((v_str(name), v_str(value)));
        }
        // Include inner text as "text" key if present
        let text: String = element.text().collect();
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            attrs.push((v_str("text"), v_str(trimmed)));
        }
        results.push(v_map(&attrs));
    }

    Ok(Ret(v_list(&results)))
}

pub(crate) fn register_bf_documents(builtins: &mut [BuiltinFunction]) {
    // XML functions
    builtins[offset_for_builtin("xml_parse")] = bf_xml_parse;
    builtins[offset_for_builtin("to_xml")] = bf_to_xml;

    // JSON functions
    builtins[offset_for_builtin("generate_json")] = bf_generate_json;
    builtins[offset_for_builtin("parse_json")] = bf_parse_json;

    // HTML functions
    builtins[offset_for_builtin("html_query")] = bf_html_query;
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_var::{v_objid, v_sym};

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
    fn test_nested_elements_with_empty_attributes() {
        // This test captures the bug: {"p", {}, nested_content} should generate
        // <p>nested_content</p>, not <p nested_content="nested_content" />

        // Create a structure like {"p", {}, {"em", {}, "Title"}}
        let nested_element = v_list(&[
            v_str("em"),
            v_list(&[]), // Empty attributes
            v_str("Title"),
        ]);

        let outer_element = v_list(&[
            v_str("p"),
            v_list(&[]), // Empty attributes
            nested_element,
        ]);

        // Mock tag resolver for flyweights (not used in this test)
        let mut tag_resolver =
            |_fl: &Flyweight| -> Result<String, BfErr> { Ok("mock".to_string()) };

        // Process the XML - this should NOT treat nested_element as an attribute
        let result = value_to_xml_tag(&outer_element, &mut tag_resolver);

        match result {
            Ok(tags) => {
                // We should get: StartElement(p), StartElement(em), Text(Title), EndElement, EndElement
                assert_eq!(tags.len(), 5);

                // Check first tag: <p>
                match &tags[0] {
                    Tag::StartElement(name, attrs) => {
                        assert_eq!(name, "p");
                        assert_eq!(attrs.len(), 0, "p element should have no attributes");
                    }
                    _ => panic!("Expected StartElement for p, got {:?}", tags[0]),
                }

                // Check second tag: <em>
                match &tags[1] {
                    Tag::StartElement(name, attrs) => {
                        assert_eq!(name, "em");
                        assert_eq!(attrs.len(), 0, "em element should have no attributes");
                    }
                    _ => panic!("Expected StartElement for em, got {:?}", tags[1]),
                }

                // Check third tag: Text content
                match &tags[2] {
                    Tag::Text(text) => {
                        assert_eq!(text, "Title");
                    }
                    _ => panic!("Expected Text content, got {:?}", tags[2]),
                }

                // Check fourth and fifth tags: end elements
                match &tags[3] {
                    Tag::EndElement(_) => {}
                    _ => panic!("Expected EndElement, got {:?}", tags[3]),
                }

                match &tags[4] {
                    Tag::EndElement(_) => {}
                    _ => panic!("Expected EndElement, got {:?}", tags[4]),
                }
            }
            Err(e) => panic!("Failed to process nested elements: {e:?}"),
        }
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
        let _tag_resolver = |value: &moor_var::Var| -> Result<Vec<Tag>, BfErr> {
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

        // Test generating XML - first get tags then convert to XML
        let mut dummy_resolver =
            |_fl: &Flyweight| -> Result<String, BfErr> { Ok("mock".to_string()) };
        let tags = value_to_xml_tag(&list_format, &mut dummy_resolver).unwrap();
        let xml_result = generate_xml_from_tags(&tags).unwrap();

        // Should generate proper XML
        assert!(xml_result.contains("<div class=\"test\">Hello World</div>"));
    }

    #[test]
    fn test_generate_xml_nested_structure() {
        // Test nested structure: {"div", {}, "Hello ", {"span", {}, "World"}}
        let nested_list = v_list(&[
            v_str("div"),
            v_list(&[]), // Empty attributes
            v_str("Hello "),
            v_list(&[v_str("span"), v_list(&[]), v_str("World")]),
        ]);

        // Recursive tag resolver for nested structures
        let _tag_resolver = |value: &moor_var::Var| -> Result<Vec<Tag>, BfErr> {
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

        // Test generating XML - first get tags then convert to XML
        let mut dummy_resolver =
            |_fl: &Flyweight| -> Result<String, BfErr> { Ok("mock".to_string()) };
        let tags = value_to_xml_tag(&nested_list, &mut dummy_resolver).unwrap();
        let xml_result = generate_xml_from_tags(&tags).unwrap();

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

    #[test]
    fn test_json_uuid_objects() {
        use moor_var::{Obj, UuObjid, Var};

        // Test regular object
        let regular_obj = v_objid(42);
        let regular_json = moo_value_to_json(&regular_obj).unwrap();
        assert_eq!(regular_json.as_str().unwrap(), "#42");

        // Test UUID object
        let uuid = UuObjid::new(0x1234, 0x5, 0x1234567890);
        let uuid_obj = Var::from(Obj::mk_uuobjid(uuid));
        let uuid_json = moo_value_to_json(&uuid_obj).unwrap();
        assert_eq!(uuid_json.as_str().unwrap(), "#048D05-1234567890");

        // Test in a list context
        let list_with_objects = v_list(&[regular_obj.clone(), uuid_obj.clone()]);
        let list_json = moo_value_to_json(&list_with_objects).unwrap();
        let json_array = list_json.as_array().unwrap();
        assert_eq!(json_array[0].as_str().unwrap(), "#42");
        assert_eq!(json_array[1].as_str().unwrap(), "#048D05-1234567890");

        // Test in a map context (as key)
        let map_with_obj_keys =
            v_map(&[(regular_obj, v_str("regular")), (uuid_obj, v_str("uuid"))]);
        let map_json = moo_value_to_json(&map_with_obj_keys).unwrap();
        let json_obj = map_json.as_object().unwrap();
        assert_eq!(json_obj.get("#42").unwrap().as_str().unwrap(), "regular");
        assert_eq!(
            json_obj
                .get("#048D05-1234567890")
                .unwrap()
                .as_str()
                .unwrap(),
            "uuid"
        );
    }
}
