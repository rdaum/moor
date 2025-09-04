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

use uuid::Uuid;

use crate::db_worldstate::DbWorldState;
use moor_common::model::ObjAttrs;
use moor_common::model::ObjSet;
use moor_common::model::PropFlag;
use moor_common::model::VerbArgsSpec;
use moor_common::model::VerbDefs;
use moor_common::model::VerbFlag;
use moor_common::model::loader::{LoaderInterface, SnapshotInterface};
use moor_common::model::{CommitResult, ObjectKind, WorldStateError};
use moor_common::model::{HasUuid, PropPerms, ValSet};
use moor_common::model::{PropDef, PropDefs};
use moor_common::util::BitEnum;
use moor_var::Obj;
use moor_var::Symbol;
use moor_var::Var;
use moor_var::program::ProgramType;

/// Implementation of LoaderInterface for write operations during loading
impl LoaderInterface for DbWorldState {
    fn create_object(
        &mut self,
        objid: Option<Obj>,
        attrs: &ObjAttrs,
    ) -> Result<Obj, WorldStateError> {
        let id_kind = match objid {
            Some(id) => ObjectKind::Objid(id),
            None => ObjectKind::NextObjid,
        };
        self.get_tx_mut().create_object(id_kind, attrs.clone())
    }
    fn set_object_parent(&mut self, obj: &Obj, parent: &Obj) -> Result<(), WorldStateError> {
        self.get_tx_mut().set_object_parent(obj, parent)
    }
    fn set_object_location(&mut self, o: &Obj, location: &Obj) -> Result<(), WorldStateError> {
        self.get_tx_mut().set_object_location(o, location)
    }
    fn set_object_owner(&mut self, obj: &Obj, owner: &Obj) -> Result<(), WorldStateError> {
        self.get_tx_mut().set_object_owner(obj, owner)
    }
    fn add_verb(
        &mut self,
        obj: &Obj,
        names: &[Symbol],
        owner: &Obj,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
        program: ProgramType,
    ) -> Result<(), WorldStateError> {
        self.get_tx_mut()
            .add_object_verb(obj, owner, names, program, flags, args)?;
        Ok(())
    }

    fn define_property(
        &mut self,
        definer: &Obj,
        objid: &Obj,
        propname: Symbol,
        owner: &Obj,
        flags: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<(), WorldStateError> {
        self.get_tx_mut()
            .define_property(definer, objid, propname, owner, flags, value)?;
        Ok(())
    }
    fn set_property(
        &mut self,
        objid: &Obj,
        propname: Symbol,
        owner: Option<Obj>,
        flags: Option<BitEnum<PropFlag>>,
        value: Option<Var>,
    ) -> Result<(), WorldStateError> {
        // First find the property.
        let (propdef, _, _, _) = self.get_tx().resolve_property(objid, propname)?;

        // Now set the value if provided.
        if let Some(value) = value {
            self.get_tx_mut()
                .set_property(objid, propdef.uuid(), value)?;
        }

        // And then set the flags and owner the child had.
        self.get_tx_mut()
            .update_property_info(objid, propdef.uuid(), owner, flags, None)?;
        Ok(())
    }

    fn max_object(&self) -> Result<Obj, WorldStateError> {
        self.get_tx().get_max_object()
    }

    fn commit(self: Box<Self>) -> Result<CommitResult, WorldStateError> {
        self.tx.commit()
    }
}

/// Implementation of SnapshotInterface for read operations during exporting
impl SnapshotInterface for DbWorldState {
    fn get_objects(&self) -> Result<ObjSet, WorldStateError> {
        self.get_tx().get_objects()
    }

    fn get_players(&self) -> Result<ObjSet, WorldStateError> {
        self.get_tx().get_players()
    }

    fn get_object(&self, objid: &Obj) -> Result<ObjAttrs, WorldStateError> {
        Ok(ObjAttrs::new(
            self.get_tx().get_object_owner(objid)?,
            self.get_tx().get_object_parent(objid)?,
            self.get_tx().get_object_location(objid)?,
            self.get_tx().get_object_flags(objid)?,
            &self.get_tx().get_object_name(objid)?,
        ))
    }

    fn get_object_verbs(&self, objid: &Obj) -> Result<VerbDefs, WorldStateError> {
        self.get_tx().get_verbs(objid)
    }

    fn get_verb_program(&self, objid: &Obj, uuid: Uuid) -> Result<ProgramType, WorldStateError> {
        self.get_tx().get_verb_program(objid, uuid)
    }

    fn get_object_properties(&self, objid: &Obj) -> Result<PropDefs, WorldStateError> {
        self.get_tx().get_properties(objid)
    }

    fn get_property_value(
        &self,
        obj: &Obj,
        uuid: Uuid,
    ) -> Result<(Option<Var>, PropPerms), WorldStateError> {
        self.get_tx().retrieve_property(obj, uuid)
    }

    #[allow(clippy::type_complexity)]
    fn get_all_property_values(
        &self,
        this: &Obj,
    ) -> Result<Vec<(PropDef, (Option<Var>, PropPerms))>, WorldStateError> {
        // First get the entire inheritance hierarchy.
        let hierarchy = self.get_tx().ancestors(this, true)?;

        // Now get the property common for each of those objects, but only for the props which
        // are defined by that object.
        // At the same time, get the common.
        let mut properties = vec![];
        for obj in hierarchy.iter() {
            let obj_propdefs = self.get_tx().get_properties(&obj)?;
            for p in obj_propdefs.iter() {
                if p.definer() != obj {
                    continue;
                }
                let value = self.get_tx().retrieve_property(this, p.uuid())?;
                properties.push((p.clone(), value));
            }
        }
        Ok(properties)
    }

    fn get_anonymous_object_metadata(
        &self,
        _objid: &Obj,
    ) -> Result<Option<Box<dyn std::any::Any + Send>>, WorldStateError> {
        // DbWorldState doesn't support GC operations, only snapshots do
        Err(WorldStateError::DatabaseError(
            "GC operations not supported on transactions, use snapshots".to_string(),
        ))
    }

    fn scan_anonymous_object_references(&self) -> Result<Vec<(Obj, Vec<Obj>)>, WorldStateError> {
        // DbWorldState doesn't support GC operations, only snapshots do
        Err(WorldStateError::DatabaseError(
            "GC operations not supported on transactions, use snapshots".to_string(),
        ))
    }
}
