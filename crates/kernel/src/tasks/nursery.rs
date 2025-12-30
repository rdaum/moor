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

//! Task-local nursery arena for short-lived anonymous objects.
//!
//! Nursery objects are anonymous objects that haven't been promoted to the database yet.
//! They live only during task execution and are either:
//! - Promoted to real AnonymousObjid when stored in a property (swizzled at commit)
//! - Discarded when the task completes (free cleanup)

use std::collections::HashMap;

use moor_var::{NurseryId, Obj, Symbol, Var};

/// Task-local storage for nursery objects.
#[derive(Debug, Default)]
pub struct Nursery {
    objects: HashMap<u32, NurseryObject>,
    next_id: u32,
}

/// An object living in nursery (not yet persisted to database).
#[derive(Debug, Clone)]
pub struct NurseryObject {
    pub parent: Obj,
    pub owner: Obj,
    pub slots: HashMap<Symbol, Var>,
}

impl Nursery {
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate a new nursery object, returning its Obj reference.
    pub fn allocate(&mut self, parent: Obj, owner: Obj) -> Obj {
        let id = self.next_id;
        self.next_id += 1;
        self.objects.insert(
            id,
            NurseryObject {
                parent,
                owner,
                slots: HashMap::new(),
            },
        );
        Obj::mk_nursery(id)
    }

    /// Get a nursery object by its ID.
    pub fn get(&self, id: NurseryId) -> Option<&NurseryObject> {
        self.objects.get(&id.0)
    }

    /// Get mutable access to a nursery object.
    pub fn get_mut(&mut self, id: NurseryId) -> Option<&mut NurseryObject> {
        self.objects.get_mut(&id.0)
    }

    /// Check if a nursery object exists.
    pub fn contains(&self, id: NurseryId) -> bool {
        self.objects.contains_key(&id.0)
    }

    /// Remove a nursery object (for promotion to real anonymous object).
    pub fn remove(&mut self, id: NurseryId) -> Option<NurseryObject> {
        self.objects.remove(&id.0)
    }

    /// Clear all nursery objects (task cleanup).
    pub fn clear(&mut self) {
        self.objects.clear();
        self.next_id = 0;
    }

    /// Number of objects in nursery.
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// Check if nursery is empty.
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }
}

// ============================================================================
// Swizzling functions for promoting nursery objects to real anonymous objects
// ============================================================================

use moor_common::model::{ObjectKind, ObjectRef, WorldState, WorldStateError, PropFlag};
use moor_common::util::BitEnum;
use moor_var::{Associative, Flyweight, Lambda, List, Sequence, v_flyweight, v_list, v_map, v_obj};

/// Check if a value contains any nursery references.
pub fn contains_nursery_refs(value: &Var) -> bool {
    use moor_var::Variant;

    match value.variant() {
        Variant::Obj(obj) => obj.is_nursery(),
        Variant::List(list) => list.iter().any(|item| contains_nursery_refs(&item)),
        Variant::Map(map) => map.iter().any(|(k, v)| contains_nursery_refs(&k) || contains_nursery_refs(&v)),
        Variant::Flyweight(fw) => {
            fw.delegate().is_nursery()
                || fw.slots().iter().any(|(_, v)| contains_nursery_refs(v))
                || fw.contents().iter().any(|item| contains_nursery_refs(&item))
        }
        Variant::Err(err) => err.value.as_ref().is_some_and(|v| contains_nursery_refs(v)),
        Variant::Lambda(lambda) => {
            lambda.0.captured_env.iter().any(|frame|
                frame.iter().any(contains_nursery_refs)
            )
        }
        _ => false,
    }
}

/// Swizzle a value, replacing nursery refs with real anonymous objects.
/// Returns the swizzled value and updates the nursery (removing promoted objects).
pub fn swizzle_value(
    value: Var,
    nursery: &mut Nursery,
    world_state: &mut dyn WorldState,
    perms: &Obj,
) -> Result<Var, WorldStateError> {
    // Use a map to track already-promoted nursery objects (for cycles/sharing)
    let mut promoted: HashMap<u32, Obj> = HashMap::new();
    swizzle_value_recursive(value, nursery, world_state, perms, &mut promoted)
}

fn swizzle_value_recursive(
    value: Var,
    nursery: &mut Nursery,
    world_state: &mut dyn WorldState,
    perms: &Obj,
    promoted: &mut HashMap<u32, Obj>,
) -> Result<Var, WorldStateError> {
    use moor_var::Variant;

    match value.variant() {
        Variant::Obj(obj) => {
            if let Some(nursery_id) = obj.nursery_id() {
                // Check if already promoted
                if let Some(anon_obj) = promoted.get(&nursery_id.0) {
                    return Ok(v_obj(*anon_obj));
                }
                // Promote this nursery object
                let anon_obj = promote_nursery_object(nursery_id, nursery, world_state, perms, promoted)?;
                Ok(v_obj(anon_obj))
            } else {
                Ok(value)
            }
        }
        Variant::List(list) => {
            let mut items: Vec<Var> = Vec::with_capacity(list.len());
            for item in list.iter() {
                items.push(swizzle_value_recursive(item, nursery, world_state, perms, promoted)?);
            }
            Ok(v_list(&items))
        }
        Variant::Map(map) => {
            let mut pairs: Vec<(Var, Var)> = Vec::with_capacity(map.len());
            for (k, v) in map.iter() {
                let k_swizzled = swizzle_value_recursive(k, nursery, world_state, perms, promoted)?;
                let v_swizzled = swizzle_value_recursive(v, nursery, world_state, perms, promoted)?;
                pairs.push((k_swizzled, v_swizzled));
            }
            Ok(v_map(&pairs))
        }
        Variant::Flyweight(fw) => {
            swizzle_flyweight(fw, nursery, world_state, perms, promoted)
        }
        Variant::Err(err) => {
            if let Some(err_value) = &err.value {
                let swizzled_value = swizzle_value_recursive(
                    (**err_value).clone(),
                    nursery,
                    world_state,
                    perms,
                    promoted,
                )?;
                // Create a new Error with the swizzled value
                let new_err = moor_var::Error {
                    err_type: err.err_type,
                    msg: err.msg.clone(),
                    value: Some(Box::new(swizzled_value)),
                };
                Ok(moor_var::v_error(new_err))
            } else {
                Ok(value)
            }
        }
        Variant::Lambda(lambda) => {
            swizzle_lambda(lambda, nursery, world_state, perms, promoted)
        }
        _ => Ok(value),
    }
}

fn swizzle_flyweight(
    fw: &Flyweight,
    nursery: &mut Nursery,
    world_state: &mut dyn WorldState,
    perms: &Obj,
    promoted: &mut HashMap<u32, Obj>,
) -> Result<Var, WorldStateError> {
    // Swizzle the delegate
    let new_delegate = if let Some(nursery_id) = fw.delegate().nursery_id() {
        if let Some(anon_obj) = promoted.get(&nursery_id.0) {
            *anon_obj
        } else {
            promote_nursery_object(nursery_id, nursery, world_state, perms, promoted)?
        }
    } else {
        *fw.delegate()
    };

    // Swizzle slots
    let mut new_slots: Vec<(Symbol, Var)> = Vec::with_capacity(fw.slots().len());
    for (name, slot_value) in fw.slots() {
        let swizzled = swizzle_value_recursive(slot_value, nursery, world_state, perms, promoted)?;
        new_slots.push((name, swizzled));
    }

    // Swizzle contents
    let mut new_contents: Vec<Var> = Vec::with_capacity(fw.contents().len());
    for item in fw.contents().iter() {
        new_contents.push(swizzle_value_recursive(item, nursery, world_state, perms, promoted)?);
    }

    Ok(v_flyweight(new_delegate, &new_slots, List::from_iter(new_contents)))
}

fn swizzle_lambda(
    lambda: &Lambda,
    nursery: &mut Nursery,
    world_state: &mut dyn WorldState,
    perms: &Obj,
    promoted: &mut HashMap<u32, Obj>,
) -> Result<Var, WorldStateError> {
    // Swizzle the captured environment
    let mut new_captured_env: Vec<Vec<Var>> = Vec::with_capacity(lambda.0.captured_env.len());
    for frame in &lambda.0.captured_env {
        let mut new_frame: Vec<Var> = Vec::with_capacity(frame.len());
        for var in frame {
            new_frame.push(swizzle_value_recursive(var.clone(), nursery, world_state, perms, promoted)?);
        }
        new_captured_env.push(new_frame);
    }

    // Create a new lambda with swizzled captured environment
    let new_lambda = Lambda::new(
        lambda.0.params.clone(),
        lambda.0.body.clone(),
        new_captured_env,
        lambda.0.self_var,
    );

    Ok(Var::from_lambda(new_lambda))
}

fn promote_nursery_object(
    nursery_id: NurseryId,
    nursery: &mut Nursery,
    world_state: &mut dyn WorldState,
    perms: &Obj,
    promoted: &mut HashMap<u32, Obj>,
) -> Result<Obj, WorldStateError> {
    // Remove from nursery to get ownership
    let nursery_obj = nursery.remove(nursery_id)
        .ok_or_else(|| WorldStateError::ObjectNotFound(ObjectRef::Id(Obj::mk_nursery(nursery_id.0))))?;

    // Create real anonymous object
    let anon_obj = world_state.create_object(
        perms,
        &nursery_obj.parent,
        &nursery_obj.owner,
        BitEnum::new(),
        ObjectKind::Anonymous,
    )?;

    // Record in promoted map before copying slots (handles cycles)
    promoted.insert(nursery_id.0, anon_obj);

    // Copy slots (swizzling any nested nursery refs)
    // For anonymous objects, we need to define the property first
    for (prop_name, prop_value) in nursery_obj.slots {
        let swizzled_value = swizzle_value_recursive(
            prop_value, nursery, world_state, perms, promoted
        )?;
        // Define the property with default flags (writable, readable, not chown-able)
        let prop_flags = BitEnum::new() | PropFlag::Read | PropFlag::Write;
        world_state.define_property(
            perms,
            &anon_obj, // definer is the anonymous object itself
            &anon_obj, // location is the anonymous object
            prop_name,
            &nursery_obj.owner, // owner comes from nursery object
            prop_flags,
            Some(swizzled_value),
        )?;
    }

    Ok(anon_obj)
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_var::NOTHING;

    #[test]
    fn test_nursery_allocate() {
        let mut nursery = Nursery::new();
        let parent = Obj::mk_id(1);
        let owner = Obj::mk_id(2);

        let obj = nursery.allocate(parent, owner);

        assert!(obj.is_nursery());
        assert_eq!(obj.nursery_id(), Some(NurseryId(0)));
        assert!(nursery.contains(NurseryId(0)));
        assert_eq!(nursery.len(), 1);
    }

    #[test]
    fn test_nursery_get() {
        let mut nursery = Nursery::new();
        let parent = Obj::mk_id(1);
        let owner = Obj::mk_id(2);

        let obj = nursery.allocate(parent, owner);
        let id = obj.nursery_id().unwrap();

        let nursery_obj = nursery.get(id).unwrap();
        assert_eq!(nursery_obj.parent, parent);
        assert_eq!(nursery_obj.owner, owner);
        assert!(nursery_obj.slots.is_empty());
    }

    #[test]
    fn test_nursery_slots() {
        let mut nursery = Nursery::new();
        let obj = nursery.allocate(NOTHING, NOTHING);
        let id = obj.nursery_id().unwrap();

        // Set a slot
        let prop_name = Symbol::mk("test_prop");
        let value = Var::mk_integer(42);
        nursery.get_mut(id)
            .unwrap()
            .slots
            .insert(prop_name, value.clone());

        // Read it back
        let nursery_obj = nursery.get(id).unwrap();
        assert_eq!(nursery_obj.slots.get(&prop_name), Some(&value));
    }

    #[test]
    fn test_nursery_clear() {
        let mut nursery = Nursery::new();
        nursery.allocate(NOTHING, NOTHING);
        nursery.allocate(NOTHING, NOTHING);

        assert_eq!(nursery.len(), 2);
        nursery.clear();
        assert!(nursery.is_empty());
        assert_eq!(nursery.next_id, 0);
    }

    #[test]
    fn test_nursery_remove() {
        let mut nursery = Nursery::new();
        let parent = Obj::mk_id(5);
        let owner = Obj::mk_id(6);
        let obj = nursery.allocate(parent, owner);
        let id = obj.nursery_id().unwrap();

        let removed = nursery.remove(id).unwrap();
        assert_eq!(removed.parent, parent);
        assert_eq!(removed.owner, owner);
        assert!(!nursery.contains(id));
        assert!(nursery.is_empty());
    }
}
