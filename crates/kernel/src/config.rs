//! Config is created by the host daemon, and passed through the scheduler, whereupon it is
//! available to all components. Used to hold things typically configured by CLI flags, etc.

use std::path::PathBuf;

#[derive(Debug, Default)]
pub struct Config {
    pub textdump_output: Option<PathBuf>,
}
