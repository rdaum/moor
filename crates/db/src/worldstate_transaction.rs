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

use byteview::ByteView;
use uuid::Uuid;

use moor_common::model::PropFlag;
use moor_common::model::VerbArgsSpec;
use moor_common::model::{BinaryType, VerbAttrs, VerbFlag};
use moor_common::model::{CommitResult, WorldStateError};
use moor_common::model::{ObjAttrs, ObjFlag};
use moor_common::model::{ObjSet, PropPerms};
use moor_common::model::{PropDef, PropDefs};
use moor_common::model::{VerbDef, VerbDefs};
use moor_common::util::BitEnum;
use moor_var::Obj;
use moor_var::Symbol;
use moor_var::Var;

/// A trait defining a generic interface to a database for storing the per-attribute common
/// of our objects and their properties and verbs.  Used by DbTxWorldState.
/// One instance per transaction.
pub trait WorldStateTransaction: Send {
    /// Check the validity of the given object.
    fn object_valid(&self, obj: &Obj) -> Result<bool, WorldStateError>;

    /// Returns all the ancestors (+ self) of the given object, in order from self to root.
    fn ancestors(&self, obj: &Obj) -> Result<ObjSet, WorldStateError>;

    /// Get the list of all objects
    fn get_objects(&self) -> Result<ObjSet, WorldStateError>;

    /// Set the flags of an object.
    fn get_object_flags(&self, obj: &Obj) -> Result<BitEnum<ObjFlag>, WorldStateError>;

    /// Get the set of all objects which are 'players' in the world.
    fn get_players(&self) -> Result<ObjSet, WorldStateError>;

    /// Get the highest "object #" in the database.
    fn get_max_object(&self) -> Result<Obj, WorldStateError>;

    /// Get the owner of the given object.
    fn get_object_owner(&self, obj: &Obj) -> Result<Obj, WorldStateError>;

    /// Set the owner of the given object.
    fn set_object_owner(&mut self, obj: &Obj, owner: &Obj) -> Result<(), WorldStateError>;

    /// Set the flags of an object.
    fn set_object_flags(
        &mut self,
        obj: &Obj,
        flags: BitEnum<ObjFlag>,
    ) -> Result<(), WorldStateError>;

    /// Get the name of the given object.
    fn get_object_name(&self, obj: &Obj) -> Result<String, WorldStateError>;

    /// Set the name of the given object.
    fn set_object_name(&mut self, obj: &Obj, name: String) -> Result<(), WorldStateError>;

    /// Create a new object, assigning it a new unique object id if one is not provided, and manage
    /// the property inheritance and ownership rules around the object.
    fn create_object(&mut self, id: Option<Obj>, attrs: ObjAttrs) -> Result<Obj, WorldStateError>;

    /// Destroy the given object, and restructure the property inheritance accordingly.
    fn recycle_object(&mut self, obj: &Obj) -> Result<(), WorldStateError>;

    /// Get the parent of the given object.
    fn get_object_parent(&self, obj: &Obj) -> Result<Obj, WorldStateError>;

    /// Set the parent of the given object, and restructure the property inheritance accordingly.
    fn set_object_parent(&mut self, obj: &Obj, parent: &Obj) -> Result<(), WorldStateError>;

    /// Get the children of the given object.
    fn get_object_children(&self, obj: &Obj) -> Result<ObjSet, WorldStateError>;

    /// Get the location of the given object.
    fn get_object_location(&self, obj: &Obj) -> Result<Obj, WorldStateError>;

    /// Get the contents of the given object.
    fn get_object_contents(&self, obj: &Obj) -> Result<ObjSet, WorldStateError>;

    /// Get the stored size of the given object & all its properties, verbs, etc.
    fn get_object_size_bytes(&self, obj: &Obj) -> Result<usize, WorldStateError>;

    /// Set the location of the given object.
    fn set_object_location(&mut self, obj: &Obj, location: &Obj) -> Result<(), WorldStateError>;

    /// Get all the verb defined on the given object.
    fn get_verbs(&self, obj: &Obj) -> Result<VerbDefs, WorldStateError>;

    /// Get the binary of the given verb.
    fn get_verb_binary(&self, obj: &Obj, uuid: Uuid) -> Result<ByteView, WorldStateError>;

    /// Find & get the verb with the given name on the given object.
    fn get_verb_by_name(&self, obj: &Obj, name: Symbol) -> Result<VerbDef, WorldStateError>;

    /// Find the Nth verb on the given object. Order is set by the time of creation.
    fn get_verb_by_index(&self, obj: &Obj, index: usize) -> Result<VerbDef, WorldStateError>;

    /// Resolve the given verb name on the given object, following the inheritance hierarchy up the
    /// chain of parents.
    fn resolve_verb(
        &self,
        obj: &Obj,
        name: Symbol,
        argspec: Option<VerbArgsSpec>,
        flagspec: Option<BitEnum<VerbFlag>>,
    ) -> Result<VerbDef, WorldStateError>;

    /// Update the provided attributes for the given verb.
    fn update_verb(
        &mut self,
        obj: &Obj,
        uuid: Uuid,
        verb_attrs: VerbAttrs,
    ) -> Result<(), WorldStateError>;

    /// Define a new verb on the given object.
    #[allow(clippy::too_many_arguments)]
    fn add_object_verb(
        &mut self,
        location: &Obj,
        owner: &Obj,
        names: Vec<Symbol>,
        binary: Vec<u8>,
        binary_type: BinaryType,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
    ) -> Result<(), WorldStateError>;

    /// Remove the given verb from the given object.
    fn delete_verb(&mut self, location: &Obj, uuid: Uuid) -> Result<(), WorldStateError>;

    /// Get the properties defined on the given object.
    fn get_properties(&self, obj: &Obj) -> Result<PropDefs, WorldStateError>;

    /// Set the property value on the given object.
    fn set_property(&mut self, obj: &Obj, uuid: Uuid, value: Var) -> Result<(), WorldStateError>;

    /// Define a new property on the given object, and propagate it to all children.
    fn define_property(
        &mut self,
        definer: &Obj,
        location: &Obj,
        name: Symbol,
        owner: &Obj,
        perms: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<Uuid, WorldStateError>;

    /// Set the property info on the given object.
    fn update_property_info(
        &mut self,
        obj: &Obj,
        uuid: Uuid,
        new_owner: Option<Obj>,
        new_flags: Option<BitEnum<PropFlag>>,
        new_name: Option<String>,
    ) -> Result<(), WorldStateError>;

    /// "Clear" the local value of the property on the given object so that it inherits from its
    /// parent.
    fn clear_property(&mut self, obj: &Obj, uuid: Uuid) -> Result<(), WorldStateError>;

    /// Delete the property from the given object, and propagate the deletion to all children.
    fn delete_property(&mut self, obj: &Obj, uuid: Uuid) -> Result<(), WorldStateError>;

    /// Retrieve the value & owner of the property without following inheritance.
    /// If the value is 'clear', the value will be None,
    fn retrieve_property(
        &self,
        obj: &Obj,
        uuid: Uuid,
    ) -> Result<(Option<Var>, PropPerms), WorldStateError>;

    /// Retrieve the owner of the property without following inheritance.
    fn retrieve_property_permissions(
        &self,
        obj: &Obj,
        uuid: Uuid,
    ) -> Result<PropPerms, WorldStateError>;

    /// Resolve the given property name on the given object, following the inheritance hierarchy up
    /// the chain of parents.
    /// Returns the resolved value and the perms (owner & flags) of the property, and whether the
    /// value was 'clear'.
    fn resolve_property(
        &self,
        obj: &Obj,
        name: Symbol,
    ) -> Result<(PropDef, Var, PropPerms, bool), WorldStateError>;

    /// Return the (rough) size of the database in bytes.
    fn db_usage(&self) -> Result<usize, WorldStateError>;

    /// Attempt to commit the transaction, returning the result of the commit.
    fn commit(self) -> Result<CommitResult, WorldStateError>;

    /// Throw away all local mutations.
    fn rollback(self) -> Result<(), WorldStateError>;

    fn descendants(&self, obj: &Obj) -> Result<ObjSet, WorldStateError>;
}
