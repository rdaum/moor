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
use fjall::UserValue;
use uuid::Uuid;

use crate::tx_management::{Error, Timestamp};
use crate::{ObjAndUUIDHolder, StringHolder};
use moor_common::model::{
    HasUuid, ObjAttrs, ObjSet, ObjectRef, PropDef, PropDefs, PropPerms, ValSet, VerbDefs,
    WorldStateError, loader::SnapshotInterface,
};
use moor_common::util::BitEnum;
use moor_var::program::ProgramType;
use moor_var::{AsByteBuffer, NOTHING, Obj, Var};

/// A snapshot-based implementation of LoaderInterface for read-only database access
pub struct SnapshotLoader {
    pub object_location_snapshot: fjall::Snapshot,
    #[allow(dead_code)]
    pub object_contents_snapshot: fjall::Snapshot,
    pub object_flags_snapshot: fjall::Snapshot,
    pub object_parent_snapshot: fjall::Snapshot,
    #[allow(dead_code)]
    pub object_children_snapshot: fjall::Snapshot,
    pub object_owner_snapshot: fjall::Snapshot,
    pub object_name_snapshot: fjall::Snapshot,
    pub object_verbdefs_snapshot: fjall::Snapshot,
    pub object_verbs_snapshot: fjall::Snapshot,
    pub object_propdefs_snapshot: fjall::Snapshot,
    pub object_propvalues_snapshot: fjall::Snapshot,
    pub object_propflags_snapshot: fjall::Snapshot,
    #[allow(dead_code)]
    pub sequences_snapshot: fjall::Snapshot,
}

impl SnapshotLoader {
    /// Helper method to decode a value from a snapshot (same logic as FjallProvider)
    fn decode<Codomain>(&self, user_value: UserValue) -> Result<(Timestamp, Codomain), Error>
    where
        Codomain: AsByteBuffer,
    {
        let result: ByteView = user_value.into();
        let ts = Timestamp(u64::from_le_bytes(result[0..8].try_into().unwrap()));
        let codomain =
            Codomain::from_bytes(result.slice(8..)).map_err(|_| Error::EncodingFailure)?;
        Ok((ts, codomain))
    }

    /// Helper method to get a value from a snapshot
    fn get_from_snapshot<Domain, Codomain>(
        &self,
        snapshot: &fjall::Snapshot,
        domain: &Domain,
    ) -> Result<Option<Codomain>, WorldStateError>
    where
        Domain: AsByteBuffer,
        Codomain: AsByteBuffer,
    {
        let key = domain
            .as_bytes()
            .map_err(|_| WorldStateError::DatabaseError("Failed to encode domain".to_string()))?;

        let Some(result) = snapshot
            .get(key)
            .map_err(|e| WorldStateError::DatabaseError(e.to_string()))?
        else {
            return Ok(None);
        };

        let (_ts, codomain) = self
            .decode::<Codomain>(result)
            .map_err(|e| WorldStateError::DatabaseError(e.to_string()))?;
        Ok(Some(codomain))
    }

    // Individual getter methods for each relation
    fn get_object_owner(&self, objid: &Obj) -> Result<Obj, WorldStateError> {
        Ok(self
            .get_from_snapshot::<Obj, Obj>(&self.object_owner_snapshot, objid)?
            .unwrap_or(NOTHING))
    }

    fn get_object_parent(&self, objid: &Obj) -> Result<Obj, WorldStateError> {
        Ok(self
            .get_from_snapshot::<Obj, Obj>(&self.object_parent_snapshot, objid)?
            .unwrap_or(NOTHING))
    }

    fn get_object_location(&self, objid: &Obj) -> Result<Obj, WorldStateError> {
        Ok(self
            .get_from_snapshot::<Obj, Obj>(&self.object_location_snapshot, objid)?
            .unwrap_or(NOTHING))
    }

    fn get_object_flags(
        &self,
        objid: &Obj,
    ) -> Result<BitEnum<moor_common::model::ObjFlag>, WorldStateError> {
        Ok(self
            .get_from_snapshot::<Obj, BitEnum<moor_common::model::ObjFlag>>(
                &self.object_flags_snapshot,
                objid,
            )?
            .unwrap_or_default())
    }

    fn get_object_name(&self, objid: &Obj) -> Result<String, WorldStateError> {
        let name_holder = self
            .get_from_snapshot::<Obj, StringHolder>(&self.object_name_snapshot, objid)?
            .ok_or(WorldStateError::ObjectNotFound(ObjectRef::Id(*objid)))?;
        Ok(name_holder.0)
    }

    fn get_verbs(&self, objid: &Obj) -> Result<VerbDefs, WorldStateError> {
        Ok(self
            .get_from_snapshot::<Obj, VerbDefs>(&self.object_verbdefs_snapshot, objid)?
            .unwrap_or(VerbDefs::empty()))
    }

    fn get_verb_program(&self, objid: &Obj, uuid: Uuid) -> Result<ProgramType, WorldStateError> {
        let key = ObjAndUUIDHolder { obj: *objid, uuid };
        self.get_from_snapshot::<ObjAndUUIDHolder, ProgramType>(&self.object_verbs_snapshot, &key)?
            .ok_or_else(|| WorldStateError::VerbNotFound(*objid, uuid.to_string()))
    }

    fn get_properties(&self, objid: &Obj) -> Result<PropDefs, WorldStateError> {
        Ok(self
            .get_from_snapshot::<Obj, PropDefs>(&self.object_propdefs_snapshot, objid)?
            .unwrap_or_else(PropDefs::empty))
    }

    fn retrieve_property(
        &self,
        obj: &Obj,
        uuid: Uuid,
    ) -> Result<(Option<Var>, PropPerms), WorldStateError> {
        let key = ObjAndUUIDHolder { obj: *obj, uuid };

        // Get property value
        let value = self
            .get_from_snapshot::<ObjAndUUIDHolder, Var>(&self.object_propvalues_snapshot, &key)?;

        // Get property permissions - if not found, this property doesn't exist on this object
        let Some(perms) = self.get_from_snapshot::<ObjAndUUIDHolder, PropPerms>(
            &self.object_propflags_snapshot,
            &key,
        )?
        else {
            return Err(WorldStateError::PropertyNotFound(*obj, uuid.to_string()));
        };

        Ok((value, perms))
    }

    /// Get the ancestor hierarchy for an object (including the object itself if include_self is true)
    fn get_ancestors(&self, obj: &Obj, include_self: bool) -> Result<ObjSet, WorldStateError> {
        let mut ancestors = Vec::new();
        let mut current = *obj;

        if include_self {
            ancestors.push(current);
        }

        // Walk up the parent chain
        while let Some(parent) =
            self.get_from_snapshot::<Obj, Obj>(&self.object_parent_snapshot, &current)?
        {
            if parent == current {
                // Avoid infinite loops in case of self-parenting
                break;
            }
            // Stop at NOTHING - don't add system objects to hierarchy
            if parent.is_nothing() {
                break;
            }
            ancestors.push(parent);
            current = parent;
        }

        Ok(ObjSet::from_iter(ancestors))
    }
}

impl SnapshotInterface for SnapshotLoader {
    fn get_objects(&self) -> Result<ObjSet, WorldStateError> {
        // Scan all objects by iterating through the object_flags relation
        let mut objects = Vec::new();

        for entry in self.object_flags_snapshot.iter() {
            let (key, _value) = entry.map_err(|e| WorldStateError::DatabaseError(e.to_string()))?;
            let obj = Obj::from_bytes(key.into()).map_err(|_| {
                WorldStateError::DatabaseError("Failed to decode object ID".to_string())
            })?;
            objects.push(obj);
        }

        Ok(ObjSet::from_iter(objects))
    }

    fn get_players(&self) -> Result<ObjSet, WorldStateError> {
        // Scan object flags to find objects with the Player flag
        let mut players = Vec::new();

        for entry in self.object_flags_snapshot.iter() {
            let (key, value) = entry.map_err(|e| WorldStateError::DatabaseError(e.to_string()))?;
            let obj = Obj::from_bytes(key.into()).map_err(|_| {
                WorldStateError::DatabaseError("Failed to decode object ID".to_string())
            })?;

            let (_ts, flags) = self
                .decode::<BitEnum<moor_common::model::ObjFlag>>(value)
                .map_err(|e| WorldStateError::DatabaseError(e.to_string()))?;

            if flags.contains(moor_common::model::ObjFlag::User) {
                players.push(obj);
            }
        }

        Ok(ObjSet::from_iter(players))
    }

    fn get_object(&self, objid: &Obj) -> Result<ObjAttrs, WorldStateError> {
        Ok(ObjAttrs::new(
            self.get_object_owner(objid)?,
            self.get_object_parent(objid)?,
            self.get_object_location(objid)?,
            self.get_object_flags(objid)?,
            &self.get_object_name(objid)?,
        ))
    }

    fn get_object_verbs(&self, objid: &Obj) -> Result<VerbDefs, WorldStateError> {
        self.get_verbs(objid)
    }

    fn get_verb_program(&self, objid: &Obj, uuid: Uuid) -> Result<ProgramType, WorldStateError> {
        self.get_verb_program(objid, uuid)
    }

    fn get_object_properties(&self, objid: &Obj) -> Result<PropDefs, WorldStateError> {
        self.get_properties(objid)
    }

    fn get_property_value(
        &self,
        obj: &Obj,
        uuid: Uuid,
    ) -> Result<(Option<Var>, PropPerms), WorldStateError> {
        self.retrieve_property(obj, uuid)
    }

    #[allow(clippy::type_complexity)]
    fn get_all_property_values(
        &self,
        this: &Obj,
    ) -> Result<Vec<(PropDef, (Option<Var>, PropPerms))>, WorldStateError> {
        // First get the entire inheritance hierarchy
        let hierarchy = self.get_ancestors(this, true).map_err(|e| {
            WorldStateError::DatabaseError(format!("Failed to get ancestors for {this}: {e}"))
        })?;

        // Now get the property definitions for each of those objects, but only for the props which
        // are defined by that object.
        let mut properties = vec![];
        for obj in hierarchy.iter() {
            // Continue through entire hierarchy, including negative ID objects (system objects)
            // This matches the working implementation behavior
            let obj_propdefs = self.get_properties(&obj).map_err(|e| {
                WorldStateError::DatabaseError(format!(
                    "Failed to get properties for {obj} (in hierarchy of {this}): {e}"
                ))
            })?;
            for p in obj_propdefs.iter() {
                if p.definer() != obj {
                    continue;
                }
                // Only include properties that actually exist on this object
                // (have permissions defined, which indicates the property was properly set up)
                match self.retrieve_property(this, p.uuid()) {
                    Ok(value) => properties.push((p.clone(), value)),
                    Err(WorldStateError::PropertyNotFound(_, _)) => {
                        // Property definition exists but property not set on this object - skip it
                        continue;
                    }
                    Err(e) => {
                        return Err(WorldStateError::DatabaseError(format!(
                            "Failed to retrieve property {} on {} (defined by {}): {}",
                            p.name(),
                            this,
                            obj,
                            e
                        )));
                    }
                }
            }
        }
        Ok(properties)
    }
}
