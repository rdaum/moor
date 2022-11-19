use crate::model::objects::Objects;
use crate::model::props::{PropDefs, Properties};
use crate::model::verbs::Verbs;

pub mod r#match;
pub mod objects;
pub mod props;
pub mod var;
pub mod verbs;

pub trait ObjDB : Objects + Properties + PropDefs + Verbs {
    fn initialize(&mut self) -> Result<(), anyhow::Error>;
    fn commit(&mut self) -> Result<(), anyhow::Error>;
    fn rollback(&mut self) -> Result<(), anyhow::Error>;
}
