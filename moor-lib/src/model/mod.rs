use thiserror::Error;

use crate::model::objects::ObjAttr;
use crate::model::verbs::Vid;
use crate::values::objid::Objid;

pub mod r#match;
pub mod objects;
pub mod permissions;
pub mod props;
pub mod verbs;
pub mod world_state;

#[derive(Error, Debug, Eq, PartialEq)]
pub enum ObjectError {
    #[error("Object not found: {0}")]
    ObjectNotFound(Objid),
    #[error("Object already exists: {0}")]
    ObjectAlreadyExists(Objid),
    #[error("Could not set/get object attribute; {0} on #{1}")]
    ObjectAttributeError(ObjAttr, Objid),

    #[error("Property not found: {0}.{1}")]
    PropertyNotFound(Objid, String),
    #[error("Property permission denied: {0}.{1}")]
    PropertyPermissionDenied(Objid, String),
    #[error("Property definition not found: {0}.{1}")]
    PropertyDefinitionNotFound(Objid, String),

    #[error("Verb not found: {0}:{1}")]
    VerbNotFound(Objid, String),
    #[error("Verb definition not {0:?}")]
    InvalidVerb(Vid),

    #[error("Invalid verb, decode error: {0}:{1}")]
    VerbDecodeError(Objid, String),
    #[error("Verb permission denied: {0}:{1}")]
    VerbPermissionDenied(Objid, String),

    #[error("Failed object match: {0}")]
    FailedMatch(String),
    #[error("Ambiguous object match: {0}")]
    AmbiguousMatch(String),

    // Catch-alls for system level object DB errors.
    #[error("Object DB error for {0}: {1}")]
    ObjectDbError(Objid, String),
    #[error("Object DB error for {0}.{1}: {2}")]
    PropertyDbError(Objid, String, String),
    #[error("Object DB error for {0}:{1}: {2}")]
    VerbDbError(Objid, String, String),
}
