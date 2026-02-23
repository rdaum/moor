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

use moor_common::model::ObjFlag;
use moor_common::util::BitEnum;
use moor_var::{NurseryId, Obj, Symbol, Var};

/// Task-local storage for nursery objects.
#[derive(Debug, Default)]
pub struct Nursery {
    objects: HashMap<u32, NurseryObject>,
    next_id: u32,
    /// Maps nursery IDs to their promoted anonymous objects.
    /// Once a nursery object is promoted, lookups return the promoted object.
    promoted: HashMap<u32, Obj>,
}

/// An object living in nursery (not yet persisted to database).
#[derive(Debug, Clone)]
pub struct NurseryObject {
    pub parent: Obj,
    pub owner: Obj,
    pub slots: HashMap<Symbol, Var>,
    pub name: Option<String>,
    pub flags: BitEnum<ObjFlag>,
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
                name: None,
                flags: BitEnum::new(),
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

    /// Check if a nursery object exists (either active or promoted).
    pub fn contains(&self, id: NurseryId) -> bool {
        self.objects.contains_key(&id.0) || self.promoted.contains_key(&id.0)
    }

    /// Check if a nursery object has been promoted to a real anonymous object.
    pub fn is_promoted(&self, id: NurseryId) -> bool {
        self.promoted.contains_key(&id.0)
    }

    /// Get the promoted anonymous object for a nursery ID, if it was promoted.
    pub fn get_promoted(&self, id: NurseryId) -> Option<Obj> {
        self.promoted.get(&id.0).copied()
    }

    /// Resolve a nursery object to its current form:
    /// - If still in nursery, returns None (caller should use get())
    /// - If promoted, returns Some(promoted_obj)
    pub fn resolve(&self, id: NurseryId) -> Option<Obj> {
        self.promoted.get(&id.0).copied()
    }

    /// Remove a nursery object (for promotion to real anonymous object).
    pub fn remove(&mut self, id: NurseryId) -> Option<NurseryObject> {
        self.objects.remove(&id.0)
    }

    /// Record that a nursery object has been promoted to a real anonymous object.
    pub fn record_promotion(&mut self, nursery_id: NurseryId, promoted_obj: Obj) {
        self.promoted.insert(nursery_id.0, promoted_obj);
    }

    /// Clear all nursery objects (task cleanup).
    pub fn clear(&mut self) {
        self.objects.clear();
        self.promoted.clear();
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
use moor_var::{
    Associative, Flyweight, Lambda, List, Sequence, NOTHING, v_bool_int, v_empty_list, v_flyweight,
    v_list, v_map, v_obj, v_str,
};

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

lazy_static::lazy_static! {
    // Implicit object attributes - exist on all objects, set via update_property
    static ref NAME_SYM: Symbol = Symbol::mk("name");
    static ref OWNER_SYM: Symbol = Symbol::mk("owner");
    static ref PROGRAMMER_SYM: Symbol = Symbol::mk("programmer");
    static ref WIZARD_SYM: Symbol = Symbol::mk("wizard");
    static ref R_SYM: Symbol = Symbol::mk("r");
    static ref W_SYM: Symbol = Symbol::mk("w");
    static ref F_SYM: Symbol = Symbol::mk("f");
    // Read-only implicit properties - cannot be set directly
    static ref LOCATION_SYM: Symbol = Symbol::mk("location");
    static ref CONTENTS_SYM: Symbol = Symbol::mk("contents");
    static ref PARENT_SYM: Symbol = Symbol::mk("parent");
    static ref CHILDREN_SYM: Symbol = Symbol::mk("children");
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImplicitProperty {
    Name,
    Owner,
    Programmer,
    Wizard,
    Read,
    Write,
    Fertile,
    Location,
    Contents,
    Parent,
    Children,
}

pub fn implicit_property_kind(name: Symbol) -> Option<ImplicitProperty> {
    if name == *NAME_SYM {
        return Some(ImplicitProperty::Name);
    }
    if name == *OWNER_SYM {
        return Some(ImplicitProperty::Owner);
    }
    if name == *PROGRAMMER_SYM {
        return Some(ImplicitProperty::Programmer);
    }
    if name == *WIZARD_SYM {
        return Some(ImplicitProperty::Wizard);
    }
    if name == *R_SYM {
        return Some(ImplicitProperty::Read);
    }
    if name == *W_SYM {
        return Some(ImplicitProperty::Write);
    }
    if name == *F_SYM {
        return Some(ImplicitProperty::Fertile);
    }
    if name == *LOCATION_SYM {
        return Some(ImplicitProperty::Location);
    }
    if name == *CONTENTS_SYM {
        return Some(ImplicitProperty::Contents);
    }
    if name == *PARENT_SYM {
        return Some(ImplicitProperty::Parent);
    }
    if name == *CHILDREN_SYM {
        return Some(ImplicitProperty::Children);
    }
    None
}

/// Check if a property name is an implicit object attribute that exists on all objects.
/// These are set via update_property, not define_property.
pub fn is_implicit_property(name: Symbol) -> bool {
    implicit_property_kind(name).is_some()
}

/// Check if a property name is read-only and should be skipped during promotion.
pub fn is_readonly_property(name: Symbol) -> bool {
    matches!(
        implicit_property_kind(name),
        Some(
            ImplicitProperty::Location
                | ImplicitProperty::Contents
                | ImplicitProperty::Parent
                | ImplicitProperty::Children
        )
    )
}

/// Return the implicit property value for a nursery object, if applicable.
pub fn implicit_property_value(nursery_obj: &NurseryObject, name: Symbol) -> Option<Var> {
    match implicit_property_kind(name) {
        Some(ImplicitProperty::Name) => {
            let name = nursery_obj.name.as_deref().unwrap_or("");
            Some(v_str(name))
        }
        Some(ImplicitProperty::Owner) => Some(v_obj(nursery_obj.owner)),
        Some(ImplicitProperty::Programmer) => {
            Some(v_bool_int(nursery_obj.flags.contains(ObjFlag::Programmer)))
        }
        Some(ImplicitProperty::Wizard) => {
            Some(v_bool_int(nursery_obj.flags.contains(ObjFlag::Wizard)))
        }
        Some(ImplicitProperty::Read) => Some(v_bool_int(nursery_obj.flags.contains(ObjFlag::Read))),
        Some(ImplicitProperty::Write) => {
            Some(v_bool_int(nursery_obj.flags.contains(ObjFlag::Write)))
        }
        Some(ImplicitProperty::Fertile) => {
            Some(v_bool_int(nursery_obj.flags.contains(ObjFlag::Fertile)))
        }
        Some(ImplicitProperty::Location) => Some(v_obj(NOTHING)),
        Some(ImplicitProperty::Contents) => Some(v_empty_list()),
        _ => None,
    }
}

fn promote_nursery_object(
    nursery_id: NurseryId,
    nursery: &mut Nursery,
    world_state: &mut dyn WorldState,
    perms: &Obj,
    in_progress: &mut HashMap<u32, Obj>,
) -> Result<Obj, WorldStateError> {
    // Check if already promoted (in the nursery's permanent record)
    if let Some(promoted_obj) = nursery.get_promoted(nursery_id) {
        return Ok(promoted_obj);
    }

    // Check if promotion is in progress (handles cycles during recursive swizzle)
    if let Some(&obj) = in_progress.get(&nursery_id.0) {
        return Ok(obj);
    }

    // Remove from nursery to get ownership
    let nursery_obj = nursery.remove(nursery_id)
        .ok_or_else(|| WorldStateError::ObjectNotFound(ObjectRef::Id(Obj::mk_nursery(nursery_id.0))))?;

    // Create real anonymous object
    let anon_obj = world_state.create_object(
        perms,
        &nursery_obj.parent,
        &nursery_obj.owner,
        nursery_obj.flags,
        ObjectKind::Anonymous,
    )?;

    // Record in both maps:
    // - in_progress: for cycle detection during this swizzle operation
    // - nursery.promoted: permanent record for remainder of task
    in_progress.insert(nursery_id.0, anon_obj);
    nursery.record_promotion(nursery_id, anon_obj);

    if let Some(name) = nursery_obj.name {
        world_state.update_property(perms, &anon_obj, *NAME_SYM, &v_str(&name))?;
    }

    // Copy slots (swizzling any nested nursery refs)
    for (prop_name, prop_value) in nursery_obj.slots {
        if is_implicit_property(prop_name) {
            continue;
        }
        let swizzled_value = swizzle_value_recursive(
            prop_value, nursery, world_state, perms, in_progress
        )?;
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
        // After remove without recording promotion, contains returns false
        assert!(!nursery.contains(id));
        assert!(nursery.is_empty());
    }

    #[test]
    fn test_nursery_promotion_tracking() {
        // This test demonstrates the fix for the post-promotion semantic issue.
        // When a nursery object is promoted (stored to a property), local references
        // that still hold the nursery ID should resolve to the promoted object.
        let mut nursery = Nursery::new();
        let obj = nursery.allocate(NOTHING, NOTHING);
        let nursery_id = obj.nursery_id().unwrap();

        // Before promotion: object exists in nursery
        assert!(nursery.contains(nursery_id));
        assert!(nursery.get(nursery_id).is_some());
        assert!(!nursery.is_promoted(nursery_id));
        assert!(nursery.get_promoted(nursery_id).is_none());

        // Simulate promotion: remove from active set and record promotion
        let _removed = nursery.remove(nursery_id).unwrap();
        let promoted_anon = Obj::mk_anonymous(moor_var::AnonymousObjid(12345)); // Simulated anonymous object ID
        nursery.record_promotion(nursery_id, promoted_anon);

        // After promotion: nursery ID is no longer in active set, but is promoted
        assert!(nursery.get(nursery_id).is_none()); // Not in active nursery anymore
        assert!(nursery.is_promoted(nursery_id));
        assert_eq!(nursery.get_promoted(nursery_id), Some(promoted_anon));

        // contains() should still return true (important for valid() checks)
        assert!(nursery.contains(nursery_id));

        // resolve() returns the promoted object
        assert_eq!(nursery.resolve(nursery_id), Some(promoted_anon));
    }

    #[test]
    fn test_nursery_clear_clears_promotions() {
        let mut nursery = Nursery::new();
        let obj = nursery.allocate(NOTHING, NOTHING);
        let nursery_id = obj.nursery_id().unwrap();

        // Remove and record promotion
        let _removed = nursery.remove(nursery_id).unwrap();
        nursery.record_promotion(nursery_id, Obj::mk_anonymous(moor_var::AnonymousObjid(999)));

        assert!(nursery.is_promoted(nursery_id));

        // Clear should remove both active objects and promotions
        nursery.clear();
        assert!(!nursery.is_promoted(nursery_id));
        assert!(!nursery.contains(nursery_id));
    }
}
