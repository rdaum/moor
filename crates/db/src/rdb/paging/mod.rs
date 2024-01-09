use thiserror::Error;

mod slotted_page;
mod tuple_box;
mod tuple_ptr;

pub use slotted_page::SlotId;
pub use tuple_box::{PageId, TupleBox};
pub use tuple_ptr::TuplePtr;

#[derive(Debug, Clone, Error)]
pub enum TupleBoxError {
    #[error("Page is full, cannot insert slot of size {0} with {1} bytes remaining")]
    BoxFull(usize, usize),
    #[error("Tuple not found at index {0}")]
    TupleNotFound(usize),
}
