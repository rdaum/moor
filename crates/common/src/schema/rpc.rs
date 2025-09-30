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

//! RPC protocol types
//!
//! Core FlatBuffer types used across the RPC layer for communication between
//! hosts, clients, workers, and the daemon.
//!
//! These types are the binary wire format for the moor distributed system.

// Re-export everything from the MoorRpc namespace
pub use crate::schema::schemas_generated::moor_rpc::*;

// Re-export common types that are frequently used with RPC types
// This provides a convenient namespace for RPC code
pub use crate::schema::common::{
    // Object types and their variants
    AnonymousObjId,
    // Error/Exception types
    CompileError,
    // Event types
    Event,
    EventRef,
    EventUnion,
    EventUnionRef,
    Exception,
    ExceptionRef,
    NarrativeEvent,
    NarrativeEventRef,
    NotifyEvent,
    Obj,
    ObjId,
    ObjRef,
    ObjUnion,
    ObjUnionRef,
    // ObjectRef types and their variants
    ObjectRef,
    ObjectRefId,
    ObjectRefMatch,
    ObjectRefRef,
    ObjectRefSysObj,
    ObjectRefUnion,
    ObjectRefUnionRef,
    PresentEvent,
    Presentation,
    PresentationAttribute,
    PresentationRef,
    // Primitive types
    Symbol,
    SymbolRef,
    TracebackEvent,
    UnpresentEvent,
    UuObjId,
    Uuid,
    UuidRef,
    VarBytes,
    VarBytesRef,
};
