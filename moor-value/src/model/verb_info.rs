use crate::model::verbdef::VerbDef;
use crate::util::slice_ref::SliceRef;
use crate::AsByteBuffer;
use bytes::BufMut;

/// The binding of both a `VerbDef` and the binary (program) associated with it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerbInfo(SliceRef);

impl VerbInfo {
    #[must_use]
    pub fn new(verbdef: VerbDef, binary: SliceRef) -> Self {
        let mut storage = Vec::new();
        // Both values we hold are variable sized, but the program is always at the end, so we can
        // just append it without a length prefix.
        verbdef.with_byte_buffer(|buffer| {
            storage.put_u32_le(buffer.len() as u32);
            storage.put_slice(buffer);
        });
        storage.put_slice(binary.as_slice());
        Self(SliceRef::from_bytes(&storage))
    }
    #[must_use]
    pub fn verbdef(&self) -> VerbDef {
        let vd_len = u32::from_le_bytes(self.0.as_slice()[0..4].try_into().unwrap()) as usize;
        VerbDef::from_sliceref(self.0.slice(4..4 + vd_len))
    }

    #[must_use]
    pub fn binary(&self) -> SliceRef {
        // The binary is after the verbdef, which is prefixed with a u32 length.
        let vd_len = u32::from_le_bytes(self.0.as_slice()[0..4].try_into().unwrap()) as usize;
        self.0.slice(4 + vd_len..)
    }
}

impl AsByteBuffer for VerbInfo {
    fn size_bytes(&self) -> usize {
        self.0.len()
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> R {
        f(self.0.as_slice())
    }

    fn make_copy_as_vec(&self) -> Vec<u8> {
        self.0.as_slice().to_vec()
    }

    fn from_sliceref(bytes: SliceRef) -> Self {
        Self(bytes)
    }
}
