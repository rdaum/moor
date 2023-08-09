#[macro_use]
extern crate pest_derive;

use lazy_static::lazy_static;

pub mod compiler;
pub mod db;
pub mod tasks;
pub mod textdump;
pub mod vm;

lazy_static! {
    pub static ref BINCODE_CONFIG: bincode::config::Configuration = bincode::config::standard();
}
