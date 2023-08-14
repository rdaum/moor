use std::fmt::{Display, Formatter};
use std::ops::RangeBounds;
use std::sync::Arc;
use yoke::Yoke;

/// A reference to a slice, along with a reference counted reference to the backing storage it came
/// from.
/// In this way it's possible to safely and conveniently pass around the slice without worrying
/// about lifetimes and borrowing.
/// This is used here for the pieces of the rope, which can all be slices out of common buffer
/// storage, and we can avoid making copies of the data when doing things like splitting nodes
/// or appending to the rope etc.
/// TODO: We need to find a way to make this work with the RocksDB DBPinnableSlice, so we can
///   go 0-zopy all the way through to the DB.
///   That will be *immensely* tricky to do without leaking Rocks details all the way up and coupling
///   us to them, so deferring for now and leaving one major unnecessary copy all the way up the
///   stack. Also not wanting to be wedded to Rocks in the long run.
#[derive(Debug, Clone)]
pub struct SliceRef(Yoke<&'static [u8], Arc<Vec<u8>>>);

impl PartialEq for SliceRef {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}
impl Eq for SliceRef {}

impl Display for SliceRef {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", String::from_utf8_lossy(self.as_slice()))
    }
}

impl SliceRef {
    pub fn empty() -> SliceRef {
        SliceRef(Yoke::attach_to_cart(Arc::new(vec![]), |b| &b[..]))
    }
    pub fn from_bytes(buf: &[u8]) -> SliceRef {
        SliceRef(Yoke::attach_to_cart(Arc::new(buf.to_vec()), |b| &b[..]))
    }
    pub fn from_vec(buf: Vec<u8>) -> SliceRef {
        SliceRef(Yoke::attach_to_cart(Arc::new(buf), |b| &b[..]))
    }
    pub fn new(buf: Arc<Vec<u8>>) -> SliceRef {
        SliceRef(Yoke::attach_to_cart(buf, |b| &b[..]))
    }
    pub fn split_at(&self, offset: usize) -> (SliceRef, SliceRef) {
        let left = SliceRef(self.0.map_project_cloned(|sl, _| &sl[..offset]));
        let right = SliceRef(self.0.map_project_cloned(|sl, _| &sl[offset..]));
        (left, right)
    }
    pub fn as_slice(&self) -> &[u8] {
        self.0.get()
    }
    pub fn len(&self) -> usize {
        self.0.get().len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.get().is_empty()
    }
    pub fn derive_empty(&self) -> SliceRef {
        SliceRef(Yoke::attach_to_cart(self.0.backing_cart().clone(), |_b| {
            &[] as &[u8]
        }))
    }

    pub fn slice<'a, R>(&'a self, range: R) -> SliceRef
    where
        R: RangeBounds<usize> + 'a + std::slice::SliceIndex<[u8], Output = [u8]>,
    {
        let result = self.0.map_project_cloned(move |sl, _| &sl[range]);
        SliceRef(result)
    }
}

#[cfg(test)]
mod tests {
    use crate::util::slice_ref::SliceRef;
    use std::sync::Arc;

    #[test]
    fn test_buffer_ref_split() {
        let backing_buffer = Arc::new(b"Hello, World!".to_vec());
        let buf = SliceRef::new(backing_buffer.clone());
        let (left, right) = buf.split_at(5);
        assert_eq!(left.as_slice(), b"Hello");
        assert_eq!(right.as_slice(), b", World!");
    }

    #[test]
    fn test_buffer_ref_slice() {
        let backing_buffer = Arc::new(b"Hello, World!".to_vec());
        let buf = SliceRef::new(backing_buffer.clone());
        assert_eq!(buf.slice(1..5).as_slice(), b"ello");
        assert_eq!(buf.slice(1..=5).as_slice(), b"ello,");
        assert_eq!(buf.slice(..5).as_slice(), b"Hello");
    }
}
