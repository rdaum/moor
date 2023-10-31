// Export TupleId & SlotBox
pub use crate::tuplebox::slots::slotbox::SlotBox;
pub use crate::tuplebox::slots::slotbox::TupleId;

mod slotbox;
mod slotted_page;

pub const TUPLEBOX_PAGE_SIZE: usize = 1 << 16;
