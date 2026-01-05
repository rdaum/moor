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

//! Object name registry for $name → object ID resolution.
//!
//! This module queries `#0` properties from a running mooR server to build
//! a mapping of symbolic names (like `$foo`) to object IDs. This allows the
//! LSP to resolve symbolic references in MOO source code.

use std::collections::HashMap;

use eyre::Result;
use moor_common::model::ObjectRef;
use moor_var::{Obj, SYSTEM_OBJECT};
use tokio::sync::RwLock;

use crate::client::MoorClient;

/// Registry of system object names ($name → Obj mapping).
///
/// Populated by querying properties on `#0` (the system object) from a mooR server.
/// Properties whose values are objects become entries in this registry.
pub struct ObjectNameRegistry {
    /// Mapping of property name to object ID.
    /// e.g., "foo" → #42 means $foo resolves to #42
    names: HashMap<String, Obj>,
}

impl ObjectNameRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            names: HashMap::new(),
        }
    }

    /// Populate the registry by querying `#0` properties from the mooR server.
    ///
    /// For each property on `#0` whose value is an object, adds an entry
    /// to the registry mapping the property name to the object ID.
    pub async fn load_from_server(client: &RwLock<MoorClient>) -> Result<Self> {
        let mut client = client.write().await;
        let mut names = HashMap::new();

        let obj_ref = ObjectRef::Id(SYSTEM_OBJECT);

        // Get all properties on #0 (non-inherited)
        let props = client.list_properties(&obj_ref, false).await?;

        // For each property, get its value and check if it's an object
        for prop in props {
            let Ok(value) = client.get_property(&obj_ref, &prop.name).await else {
                continue;
            };

            // If the value is an object, add it to our registry
            if let Some(obj) = value.as_object() {
                names.insert(prop.name.clone(), obj);
            }
        }

        Ok(Self { names })
    }

    /// Resolve a symbolic name to an object ID.
    ///
    /// The name should be without the `$` prefix (e.g., "foo" not "$foo").
    pub fn resolve(&self, name: &str) -> Option<Obj> {
        self.names.get(name).copied()
    }

    /// Get all registered names.
    #[allow(dead_code)]
    pub fn all(&self) -> &HashMap<String, Obj> {
        &self.names
    }

    /// Number of registered names.
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Check if registry is empty.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

impl Default for ObjectNameRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_registry() {
        let registry = ObjectNameRegistry::new();
        assert_eq!(registry.len(), 0);
        assert!(registry.resolve("foo").is_none());
    }

    #[test]
    fn test_registry_with_entries() {
        let mut names = HashMap::new();
        names.insert("room".to_string(), Obj::mk_id(1));
        names.insert("player".to_string(), Obj::mk_id(2));
        names.insert("thing".to_string(), Obj::mk_id(3));

        let registry = ObjectNameRegistry { names };

        assert_eq!(registry.len(), 3);
        assert_eq!(registry.resolve("room"), Some(Obj::mk_id(1)));
        assert_eq!(registry.resolve("player"), Some(Obj::mk_id(2)));
        assert_eq!(registry.resolve("thing"), Some(Obj::mk_id(3)));
        assert!(registry.resolve("nonexistent").is_none());
    }

    #[test]
    fn test_all_entries() {
        let mut names = HashMap::new();
        names.insert("foo".to_string(), Obj::mk_id(42));
        names.insert("bar".to_string(), Obj::mk_id(99));

        let registry = ObjectNameRegistry { names };
        let all = registry.all();

        assert_eq!(all.len(), 2);
        assert_eq!(all.get("foo"), Some(&Obj::mk_id(42)));
        assert_eq!(all.get("bar"), Some(&Obj::mk_id(99)));
    }
}
