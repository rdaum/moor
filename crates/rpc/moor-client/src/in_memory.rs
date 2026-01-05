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

//! In-memory world state access for LSP and tooling.
//!
//! Provides direct access to a MoorDB without network RPC, enabling the LSP
//! to work with a local database without requiring a running mooR daemon.
//!
//! # Usage
//!
//! ```ignore
//! use moor_client::in_memory::{InMemoryConfig, InMemoryWorldState};
//! use moor_var::Obj;
//!
//! // Create ephemeral in-memory database
//! let config = InMemoryConfig {
//!     db_path: None,
//!     perms_player: Obj::mk_id(0), // #0 system object for full access
//! };
//!
//! let world_state = InMemoryWorldState::new(config)?;
//! ```

use eyre::Result;
use moor_common::model::{
    HasUuid, PropDef, PropPerms, ValSet, VerbDef, WorldState, WorldStateError, WorldStateSource,
};
use moor_db::{DatabaseConfig, TxDB};
use moor_var::program::ProgramType;
use moor_var::{Obj, Symbol, Var};
use std::path::PathBuf;
use std::sync::Arc;

use crate::traits::{IntrospectionError, IntrospectionResult, MoorIntrospection};

/// Configuration for in-memory world state.
pub struct InMemoryConfig {
    /// Path to existing database, or None for ephemeral in-memory database.
    pub db_path: Option<PathBuf>,
    /// Player object for permission checks (wizard for full access).
    pub perms_player: Obj,
}

/// In-memory world state wrapper.
///
/// Wraps a MoorDB instance and provides read-only introspection methods
/// for tooling such as the LSP server. This enables working with a
/// local database without the overhead of network RPC.
pub struct InMemoryWorldState {
    /// The underlying database instance.
    db: Arc<TxDB>,
    /// The player object used for permission checks.
    perms: Obj,
}

impl InMemoryWorldState {
    /// Create a new in-memory world state.
    ///
    /// If `config.db_path` is `None`, creates an ephemeral in-memory database.
    /// If a path is provided, opens or creates a database at that location.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened.
    pub fn new(config: InMemoryConfig) -> Result<Self> {
        let path = config.db_path.as_deref();
        let (db, _fresh) = TxDB::open(path, DatabaseConfig::default());

        Ok(Self {
            db: Arc::new(db),
            perms: config.perms_player,
        })
    }

    /// Get a fresh world state handle.
    fn world_state(&self) -> IntrospectionResult<Box<dyn WorldState>> {
        self.db
            .new_world_state()
            .map_err(|e| IntrospectionError::DatabaseError(e.to_string()))
    }

    /// Convert WorldStateError to IntrospectionError.
    fn map_error(obj: &Obj, e: WorldStateError) -> IntrospectionError {
        match e {
            WorldStateError::ObjectNotFound(_) => IntrospectionError::ObjectNotFound(*obj),
            WorldStateError::VerbNotFound(_, name) => IntrospectionError::VerbNotFound(name),
            WorldStateError::PropertyNotFound(_, name)
            | WorldStateError::PropertyDefinitionNotFound(_, name) => {
                IntrospectionError::PropertyNotFound(name)
            }
            WorldStateError::ObjectPermissionDenied
            | WorldStateError::PropertyPermissionDenied
            | WorldStateError::VerbPermissionDenied => IntrospectionError::PermissionDenied,
            _ => IntrospectionError::DatabaseError(e.to_string()),
        }
    }

    /// Collect verbs from a single object.
    fn collect_verbs_from(
        &self,
        ws: &dyn WorldState,
        obj: &Obj,
        verbs: &mut Vec<VerbDef>,
    ) -> IntrospectionResult<()> {
        let obj_verbs = ws
            .verbs(&self.perms, obj)
            .map_err(|e| Self::map_error(obj, e))?;
        verbs.extend(obj_verbs.iter());
        Ok(())
    }

    /// Collect properties from a single object.
    fn collect_props_from(
        &self,
        ws: &dyn WorldState,
        obj: &Obj,
        props: &mut Vec<(PropDef, PropPerms)>,
    ) -> IntrospectionResult<()> {
        let obj_props = ws
            .properties(&self.perms, obj)
            .map_err(|e| Self::map_error(obj, e))?;

        for prop_def in obj_props.iter() {
            let prop_info = ws
                .get_property_info(&self.perms, obj, prop_def.name())
                .map_err(|e| Self::map_error(obj, e))?;
            props.push(prop_info);
        }
        Ok(())
    }

    /// Walk up the inheritance chain, calling `f` for each valid object.
    /// Stops when reaching an invalid object or detecting a cycle.
    fn walk_inheritance<F>(&self, ws: &dyn WorldState, start: &Obj, mut f: F) -> IntrospectionResult<()>
    where
        F: FnMut(&Obj) -> IntrospectionResult<()>,
    {
        let mut current = *start;

        loop {
            let is_valid = ws
                .valid(&current)
                .map_err(|e| Self::map_error(&current, e))?;

            if !is_valid {
                break;
            }

            f(&current)?;

            let parent = ws
                .parent_of(&self.perms, &current)
                .map_err(|e| Self::map_error(&current, e))?;

            if !parent.is_valid_object() || parent == current {
                break;
            }
            current = parent;
        }

        Ok(())
    }
}

impl MoorIntrospection for InMemoryWorldState {
    fn list_verbs(&self, obj: &Obj, include_inherited: bool) -> IntrospectionResult<Vec<VerbDef>> {
        let ws = self.world_state()?;
        let mut verbs = Vec::new();

        if include_inherited {
            self.walk_inheritance(&*ws, obj, |current| {
                self.collect_verbs_from(&*ws, current, &mut verbs)
            })?;
        } else {
            self.collect_verbs_from(&*ws, obj, &mut verbs)?;
        }

        Ok(verbs)
    }

    fn list_properties(
        &self,
        obj: &Obj,
        include_inherited: bool,
    ) -> IntrospectionResult<Vec<(PropDef, PropPerms)>> {
        let ws = self.world_state()?;
        let mut props = Vec::new();

        if include_inherited {
            self.walk_inheritance(&*ws, obj, |current| {
                self.collect_props_from(&*ws, current, &mut props)
            })?;
        } else {
            self.collect_props_from(&*ws, obj, &mut props)?;
        }

        Ok(props)
    }

    fn get_verb(&self, obj: &Obj, verb_name: &str) -> IntrospectionResult<(VerbDef, ProgramType)> {
        let ws = self.world_state()?;
        let verb_symbol: Symbol = verb_name.into();

        let verb_def = ws
            .get_verb(&self.perms, obj, verb_symbol)
            .map_err(|e| Self::map_error(obj, e))?;

        let (program, _) = ws
            .retrieve_verb(&self.perms, obj, verb_def.uuid())
            .map_err(|e| Self::map_error(obj, e))?;

        Ok((verb_def, program))
    }

    fn get_property(&self, obj: &Obj, prop_name: &str) -> IntrospectionResult<Var> {
        let ws = self.world_state()?;
        let prop_symbol: Symbol = prop_name.into();

        ws.retrieve_property(&self.perms, obj, prop_symbol)
            .map_err(|e| Self::map_error(obj, e))
    }

    fn get_property_info(
        &self,
        obj: &Obj,
        prop_name: &str,
    ) -> IntrospectionResult<(PropDef, PropPerms)> {
        let ws = self.world_state()?;
        let prop_symbol: Symbol = prop_name.into();

        ws.get_property_info(&self.perms, obj, prop_symbol)
            .map_err(|e| Self::map_error(obj, e))
    }

    fn list_objects(&self) -> IntrospectionResult<Vec<Obj>> {
        let ws = self.world_state()?;

        let obj_set = ws
            .all_objects()
            .map_err(|e| IntrospectionError::DatabaseError(e.to_string()))?;

        Ok(obj_set.iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_ephemeral() {
        let config = InMemoryConfig {
            db_path: None,
            perms_player: Obj::mk_id(0),
        };
        let ws = InMemoryWorldState::new(config);
        assert!(ws.is_ok());
    }
}
