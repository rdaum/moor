use crate::program::program::Program;
use bincode::{Decode, Encode};
use moor_var::BincodeAsByteBufferExt;

pub mod builtins;
pub mod labels;
pub mod names;
pub mod opcode;

#[allow(clippy::module_inception)]
pub mod program;

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub enum ProgramType {
    MooR(Program),
}

impl BincodeAsByteBufferExt for ProgramType {}

impl ProgramType {
    pub fn is_empty(&self) -> bool {
        match self {
            ProgramType::MooR(p) => p.main_vector().is_empty(),
        }
    }
}
