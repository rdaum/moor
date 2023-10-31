use crate::tuplebox::slots::{SlotBox, TupleId};
use moor_values::util::slice_ref::ByteSource;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
pub use tuple::Tuple;
pub use tx_tuple::{TupleError, TxTuple};

mod tuple;
mod tx_tuple;

pub struct TupleRef {
    sb: Arc<SlotBox>,
    id: TupleId,
}

impl PartialEq for TupleRef {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for TupleRef {}

impl Hash for TupleRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let slc = self.sb.get(self.id).expect("Unable to get tuple");
        slc.hash(state)
    }
}
impl TupleRef {
    pub fn new(sb: Arc<SlotBox>, id: TupleId) -> Self {
        sb.upcount(id).expect("Unable to add tuple");
        Self { sb, id }
    }

    pub fn get(&self) -> Tuple {
        Tuple::from_tuple_id(self.sb.clone(), self.id)
    }
}

impl Drop for TupleRef {
    fn drop(&mut self) {
        self.sb.dncount(self.id).expect("Unable to remove tuple");
    }
}

impl Clone for TupleRef {
    fn clone(&self) -> Self {
        self.sb.upcount(self.id).expect("Unable to add tuple");
        Self {
            sb: self.sb.clone(),
            id: self.id,
        }
    }
}

impl ByteSource for TupleRef {
    fn as_slice(&self) -> &[u8] {
        self.sb.get(self.id).expect("Unable to get tuple").get_ref()
    }

    fn len(&self) -> usize {
        self.as_slice().len()
    }

    fn touch(&self) {
        // noop
    }
}
