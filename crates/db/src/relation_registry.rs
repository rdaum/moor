// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

//! Logical relation schema, index policy, and mutation category.

macro_rules! relation_registry {
    ($callback:ident) => {
        $callback! {
            object_location Ordinary Strict == Obj, Obj,
            object_parent Ordinary Strict == Obj, Obj,
            object_flags Ordinary Strict => Obj, BitEnum<ObjFlag>,
            object_owner Ordinary Strict == Obj, Obj,
            object_name Ordinary Strict => Obj, StringHolder,
            object_verbdefs Ordinary Strict => Obj, VerbDefs,
            object_verbs Ordinary Strict => ObjAndUUIDHolder, ProgramType,
            object_propdefs Ordinary Strict => Obj, PropDefs,
            object_propvalues PropertyValueChain PropertyPermissions => ObjAndUUIDHolder, Var,
            object_propflags Ordinary Strict => ObjAndUUIDHolder, PropPerms,
            entity_metadata Ordinary Strict => EntityMetadataKey, Var,
            object_last_move Ordinary Strict => Obj, Var,
            anonymous_object_metadata Ordinary Strict => Obj, AnonymousObjectMetadata,
        }
    };
}
pub(crate) use relation_registry;
