use tokio::sync::oneshot::Sender;
use uuid::Uuid;

use moor_value::model::objects::{ObjAttrs, ObjFlag};
use moor_value::model::objset::ObjSet;
use moor_value::model::propdef::{PropDef, PropDefs};
use moor_value::model::props::PropFlag;
use moor_value::model::r#match::VerbArgsSpec;
use moor_value::model::verbdef::VerbDef;
use moor_value::model::verbs::{BinaryType, VerbFlag};
use moor_value::model::CommitResult;
use moor_value::model::WorldStateError;
use moor_value::util::bitenum::BitEnum;
use moor_value::var::objid::Objid;
use moor_value::var::Var;

use moor_value::model::verbdef::VerbDefs;

/// The set of messages that DbTxWorldState sends to the underlying physical database to execute
/// storage/retrieval of object attributes, properties, and verbs.
pub(crate) enum DbMessage {
    CreateObject {
        id: Option<Objid>,
        attrs: ObjAttrs,
        reply: Sender<Result<Objid, WorldStateError>>,
    },
    GetLocationOf(Objid, Sender<Result<Objid, WorldStateError>>),
    GetContentsOf(Objid, Sender<Result<ObjSet, WorldStateError>>),
    SetLocationOf(Objid, Objid, Sender<Result<(), WorldStateError>>),
    GetObjectFlagsOf(Objid, Sender<Result<BitEnum<ObjFlag>, WorldStateError>>),
    SetObjectFlagsOf(Objid, BitEnum<ObjFlag>, Sender<Result<(), WorldStateError>>),
    GetObjectNameOf(Objid, Sender<Result<String, WorldStateError>>),
    SetObjectNameOf(Objid, String, Sender<Result<(), WorldStateError>>),
    GetParentOf(Objid, Sender<Result<Objid, WorldStateError>>),
    SetParent(Objid, Objid, Sender<Result<(), WorldStateError>>),
    GetChildrenOf(Objid, Sender<Result<ObjSet, WorldStateError>>),
    GetObjectOwner(Objid, Sender<Result<Objid, WorldStateError>>),
    SetObjectOwner(Objid, Objid, Sender<Result<(), WorldStateError>>),

    /// Get information about all verbs declared on a given object
    GetVerbs(Objid, Sender<Result<VerbDefs, WorldStateError>>),
    /// Get information about a specific verb on a given object by one of its names
    GetVerbByName(Objid, String, Sender<Result<VerbDef, WorldStateError>>),
    /// Get information about a specific verb on a given object by its index in the list of verbs
    GetVerbByIndex(Objid, usize, Sender<Result<VerbDef, WorldStateError>>),
    /// Get the (binary) program for a specific verb on a given object by its unique id
    GetVerbBinary(Objid, Uuid, Sender<Result<Vec<u8>, WorldStateError>>),
    /// Search the inheritance hierarchy of an object to find a verb by name & argspec
    /// (If argspec is not specified, then "this none this" is assumed.)
    ResolveVerb {
        location: Objid,
        name: String,
        argspec: Option<VerbArgsSpec>,
        reply: Sender<Result<VerbDef, WorldStateError>>,
    },
    /// Update (non-program) data about a verb.
    UpdateVerbDef {
        obj: Objid,
        uuid: Uuid,
        owner: Option<Objid>,
        names: Option<Vec<String>>,
        flags: Option<BitEnum<VerbFlag>>,
        binary_type: Option<BinaryType>,
        args: Option<VerbArgsSpec>,
        reply: Sender<Result<(), WorldStateError>>,
    },
    /// Update the program for a verb.
    SetVerbBinary {
        obj: Objid,
        uuid: Uuid,
        binary: Vec<u8>,
        reply: Sender<Result<(), WorldStateError>>,
    },
    /// Add a verb on an object
    AddVerb {
        location: Objid,
        owner: Objid,
        names: Vec<String>,
        binary_type: BinaryType,
        binary: Vec<u8>,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
        reply: Sender<Result<(), WorldStateError>>,
    },
    /// Delete a verb from an object
    DeleteVerb {
        location: Objid,
        uuid: Uuid,
        reply: Sender<Result<(), WorldStateError>>,
    },

    /// Retrieve the list of properties defined on this object.
    GetProperties(Objid, Sender<Result<PropDefs, WorldStateError>>),
    /// Retrieve a specific property by its unique id.
    RetrieveProperty(Objid, Uuid, Sender<Result<Var, WorldStateError>>),
    /// Set a property's value by its id.
    SetProperty(Objid, Uuid, Var, Sender<Result<(), WorldStateError>>),
    /// Define a new property on an object.
    DefineProperty {
        definer: Objid,
        location: Objid,
        name: String,
        owner: Objid,
        perms: BitEnum<PropFlag>,
        value: Option<Var>,
        reply: Sender<Result<Uuid, WorldStateError>>,
    },
    /// Update information about an existing property.
    SetPropertyInfo {
        obj: Objid,
        uuid: Uuid,
        new_owner: Option<Objid>,
        new_flags: Option<BitEnum<PropFlag>>,
        new_name: Option<String>,
        reply: Sender<Result<(), WorldStateError>>,
    },
    ClearProperty(Objid, Uuid, Sender<Result<(), WorldStateError>>),
    DeleteProperty(Objid, Uuid, Sender<Result<(), WorldStateError>>),
    ResolveProperty(
        Objid,
        String,
        Sender<Result<(PropDef, Var), WorldStateError>>,
    ),
    Valid(Objid, Sender<bool>),
    Commit(Sender<CommitResult>),
    Rollback(Sender<()>),
}
