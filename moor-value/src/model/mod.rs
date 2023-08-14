use anyhow::bail;

use thiserror::Error;

use crate::model::objects::ObjAttr;
use crate::model::verbs::Vid;
use crate::var::error::Error;
use crate::var::objid::Objid;

pub mod defset;
pub mod r#match;
pub mod objects;
pub mod objset;
pub mod permissions;
pub mod propdef;
pub mod props;
pub mod verb_info;
pub mod verbdef;
pub mod verbs;
pub mod world_state;

/// The result code from a commit/complete operation on the world's state.
pub enum CommitResult {
    Success, // Value was committed
    ConflictRetry, // Value was not committed due to conflict, caller should abort and retry tx
             // TODO: timeout/task-too-long/error?
}

#[derive(Error, Debug, Eq, PartialEq)]
pub enum WorldStateError {
    #[error("Object not found: {0}")]
    ObjectNotFound(Objid),
    #[error("Object already exists: {0}")]
    ObjectAlreadyExists(Objid),
    #[error("Could not set/get object attribute; {0} on {1}")]
    ObjectAttributeError(ObjAttr, Objid),
    #[error("Recursive move detected: {0} -> {1}")]
    RecursiveMove(Objid, Objid),

    #[error("Object permission denied")]
    ObjectPermissionDenied,

    #[error("Property not found: {0}.{1}")]
    PropertyNotFound(Objid, String),
    #[error("Property permission denied")]
    PropertyPermissionDenied,
    #[error("Property definition not found: {0}.{1}")]
    PropertyDefinitionNotFound(Objid, String),
    #[error("Duplicate property definition: {0}.{1}")]
    DuplicatePropertyDefinition(Objid, String),
    #[error("Property type mismatch")]
    PropertyTypeMismatch,

    #[error("Verb not found: {0}:{1}")]
    VerbNotFound(Objid, String),
    #[error("Verb definition not {0:?}")]
    InvalidVerb(Vid),

    #[error("Invalid verb, decode error: {0}:{1}")]
    VerbDecodeError(Objid, String),
    #[error("Verb permission denied")]
    VerbPermissionDenied,
    #[error("Verb already exists: {0}:{1}")]
    DuplicateVerb(Objid, String),

    #[error("Failed object match: {0}")]
    FailedMatch(String),
    #[error("Ambiguous object match: {0}")]
    AmbiguousMatch(String),

    // Catch-alls for system level object DB errors.
    #[error("DB communications error: {0}")]
    CommunicationError(String),
}

impl WorldStateError {
    pub fn to_error_code(&self) -> Result<Error, anyhow::Error> {
        match self {
            WorldStateError::ObjectNotFound(_) => Ok(Error::E_INVARG),
            WorldStateError::ObjectPermissionDenied => Ok(Error::E_PERM),
            WorldStateError::RecursiveMove(_, _) => Ok(Error::E_RECMOVE),
            WorldStateError::VerbNotFound(_, _) => Ok(Error::E_VERBNF),
            WorldStateError::VerbPermissionDenied => Ok(Error::E_PERM),
            WorldStateError::InvalidVerb(_) => Ok(Error::E_VERBNF),
            WorldStateError::DuplicateVerb(_, _) => Ok(Error::E_INVARG),
            WorldStateError::PropertyNotFound(_, _) => Ok(Error::E_PROPNF),
            WorldStateError::PropertyPermissionDenied => Ok(Error::E_PERM),
            WorldStateError::PropertyDefinitionNotFound(_, _) => Ok(Error::E_PROPNF),
            WorldStateError::DuplicatePropertyDefinition(_, _) => Ok(Error::E_INVARG),
            WorldStateError::PropertyTypeMismatch => Ok(Error::E_TYPE),
            _ => {
                bail!("Unhandled error code: {:?}", self);
            }
        }
    }
}

// TODO: not sure this is the most appropriate place; used to be in tasks/command_parse.rs, but
// is needed elsewhere (by verb_args, etc)
// Putting here in DB because it's kinda version/DB specific, but not sure it's the best place.
pub const PREP_LIST: [&str; 15] = [
    "with/using",
    "at/to",
    "in front of",
    "in/inside/into",
    "on top of/on/onto/upon",
    "out of/from inside/from",
    "over",
    "through",
    "under/underneath/beneath",
    "behind",
    "beside",
    "for/about",
    "is",
    "as",
    "off/off of",
];
