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

use std::collections::BTreeMap;

use crate::{
    Object, Propval, Textdump, VF_ASPEC_ANY, VF_ASPEC_NONE, VF_ASPEC_THIS, VF_DOBJSHIFT,
    VF_IOBJSHIFT, Verb, Verbdef,
};
use moor_common::model::loader::LoaderInterface;
use moor_common::model::{ArgSpec, PrepSpec, ValSet, VerbArgsSpec};
use moor_common::model::{BinaryType, VerbFlag};
use moor_common::model::{HasUuid, Named};
use moor_common::util::BitEnum;
use moor_compiler::Program;
use moor_var::v_none;
use moor_var::{AsByteBuffer, NOTHING};

/// Convert verbargs spec to flags & preps accordingly
fn cv_arg(flags: BitEnum<VerbFlag>, arg: VerbArgsSpec) -> (u16, i16) {
    let flags = flags.to_u16();
    let dobjflags = match arg.dobj {
        ArgSpec::None => VF_ASPEC_NONE,
        ArgSpec::Any => VF_ASPEC_ANY,
        ArgSpec::This => VF_ASPEC_THIS,
    };
    let iobjflags = match arg.iobj {
        ArgSpec::None => VF_ASPEC_NONE,
        ArgSpec::Any => VF_ASPEC_ANY,
        ArgSpec::This => VF_ASPEC_THIS,
    };
    let prepflags = match arg.prep {
        PrepSpec::None => -1,
        PrepSpec::Any => -2,
        PrepSpec::Other(p) => p as i16,
    };

    let arg_flags = (dobjflags << VF_DOBJSHIFT) | (iobjflags << VF_IOBJSHIFT);
    (flags | arg_flags, prepflags)
}

/// Take a transaction, and scan the relations and build a Textdump representing a snapshot of the world as it
/// exists in the transaction.
pub fn make_textdump(tx: &dyn LoaderInterface, version: String) -> Textdump {
    // To create the objects list, we need to scan all objects.
    // For now, the expectation would be we can simply iterate from 0 to max object, checking validity of each
    // object, and then adding it to the list.

    // Find all the ids
    let object_ids = tx.get_objects().expect("Failed to get objects");

    // Retrieve all the objects
    let mut db_objects = BTreeMap::new();
    for id in object_ids.iter() {
        db_objects.insert(
            id.clone(),
            tx.get_object(&id).expect("Failed to get object"),
        );
    }

    // Build a map of parent -> children
    let mut children_map = BTreeMap::new();
    for id in db_objects.keys() {
        let obj = db_objects.get(id).expect("Failed to get object");
        let parent = obj.parent().unwrap_or(NOTHING);
        children_map
            .entry(parent)
            .or_insert_with(Vec::new)
            .push(id.clone());
    }

    // Same with location -> contents
    let mut contents_map = BTreeMap::new();
    for id in db_objects.keys() {
        let obj = db_objects.get(id).expect("Failed to get object");
        let location = obj.location().unwrap_or(NOTHING);
        contents_map
            .entry(location.clone())
            .or_insert_with(Vec::new)
            .push(id.clone());
    }

    // Objid -> Object
    let mut objects = BTreeMap::new();

    // (Objid, usize) -> Verb, where usize is the verb number (0-indexed)
    let mut verbs = BTreeMap::new();

    for (db_objid, db_obj) in db_objects.iter() {
        // To find 'next' for contents, we seek the contents of our location, and find the object right after
        // the current object in that vector
        let location = db_obj.location().unwrap_or(NOTHING);

        let next = if location != NOTHING {
            let roommates = contents_map
                .get_mut(&location)
                .expect("Failed to get contents");

            let position = roommates
                .iter()
                .position(|x| x == db_objid)
                .expect("Failed to find object in contents of location");
            // If position is at the end, 'next' is -1.
            if position == roommates.len() - 1 {
                NOTHING
            } else {
                roommates[position + 1].clone()
            }
        } else {
            NOTHING
        };

        // To find 'contents' we're looking for the first object whose location is the current object
        let contents = match contents_map.get_mut(db_objid) {
            Some(contents) => contents.first().unwrap_or(&NOTHING).clone(),
            None => NOTHING,
        };

        let parent = db_obj.parent().unwrap_or(NOTHING);

        // Same for 'sibling' using children/parent
        let siblings = children_map
            .get_mut(&parent)
            .expect("Failed to get siblings");
        let position = siblings
            .iter()
            .position(|x| x == db_objid)
            .expect("Failed to find object in siblings");
        let sibling = if position == siblings.len() - 1 {
            NOTHING
        } else {
            siblings[position + 1].clone()
        };

        // To find child, we need to find the first object whose parent is the current object
        let child = match children_map.get_mut(db_objid) {
            Some(children) => children.first().unwrap_or(&NOTHING).clone(),
            None => NOTHING,
        };

        // Find the verbdefs and transform them into textdump verbdefs
        let db_verbdefs = tx.get_object_verbs(db_objid).expect("Failed to get verbs");
        let verbdefs = db_verbdefs
            .iter()
            .map(|db_verbdef| {
                let name = db_verbdef.names().join(" ");
                let owner = db_verbdef.owner();
                let (flags, prep) = cv_arg(db_verbdef.flags(), db_verbdef.args());
                Verbdef {
                    name,
                    owner,
                    flags,
                    prep,
                }
            })
            .collect();
        // Produce the verbmap
        for (verbnum, verb) in db_verbdefs.iter().enumerate() {
            // Get and decompile the binary. We only support MOO for now.
            if verb.binary_type() != BinaryType::LambdaMoo18X {
                panic!("Unsupported binary type: {:?}", verb.binary_type());
            }

            let binary = tx
                .get_verb_binary(db_objid, verb.uuid())
                .expect("Failed to get verb binary");

            let program = if !binary.is_empty() {
                let program = Program::from_bytes(binary).expect("Failed to parse verb binary");
                if !program.main_vector.is_empty() {
                    let ast = moor_compiler::program_to_tree(&program)
                        .expect("Failed to decompile verb binary");
                    let program =
                        moor_compiler::unparse(&ast).expect("Failed to decompile verb binary");
                    Some(program.join("\n"))
                } else {
                    None
                }
            } else {
                None
            };

            let objid = db_objid;
            verbs.insert(
                (objid.clone(), verbnum),
                Verb {
                    objid: objid.clone(),
                    verbnum,
                    program,
                },
            );
        }

        // propvals have wonky logic which resolve relative to position in the inheritance hierarchy of
        // propdefs up to the root. So we grab that all from the loader_client, and then we can just
        // iterate through them all.
        let properties = tx.get_all_property_values(db_objid).unwrap();

        let mut propdefs = vec![];
        for (p, _) in &properties {
            if p.definer() != *db_objid {
                break;
            }
            propdefs.push(p.name().into());
        }

        let mut propvals = vec![];
        for (_, (pval, perms)) in properties {
            let owner = perms.owner();
            let flags = perms.flags().to_u16() as u8;
            let is_clear = pval.is_none();
            propvals.push(Propval {
                value: pval.unwrap_or(v_none()),
                owner,
                flags,
                is_clear,
            });
        }

        // To construct the child linkage list, we need to scan all objects, and find all objects whose parent
        // is the current object, and add them to the list.
        let obj = Object {
            id: db_objid.clone(),
            owner: db_obj.owner().unwrap(),
            location: db_obj.location().unwrap_or(NOTHING),
            contents,
            next,
            parent,
            child,
            sibling,
            name: db_obj.name().clone().unwrap_or("".to_string()),
            flags: db_obj.flags().to_u16() as _,
            verbdefs,
            propdefs,
            propvals,
        };

        objects.insert(db_objid.clone(), obj);
    }

    let users = tx
        .get_players()
        .expect("Failed to get players list")
        .iter()
        .collect();

    Textdump {
        version_string: version,
        objects,
        users,
        verbs,
    }
}
