use bincode::de::read::Reader;
use bincode::enc::write::Writer;
use bincode::error::{DecodeError, EncodeError};
use bincode::{Decode, Encode};
use bytes::{Buf, Bytes};
use lazy_static::lazy_static;

pub mod model;
pub mod util;
pub mod var;

lazy_static! {
    static ref BINCODE_CONFIG: bincode::config::Configuration = bincode::config::standard();
}

/// A trait for all values that can be stored in the database. (e.g. all of them).
/// To abstract away from the underlying serialization format, we use this trait.
pub trait AsByteBuffer {
    /// Returns the size of this value in bytes.
    /// For now assume this is a costly operation.
    fn size_bytes(&self) -> usize;
    /// Return the bytes representing this value.
    fn as_byte_buffer(&self) -> Bytes;
    /// Create a value from the given bytes.
    /// Either takes ownership or moves.
    fn from_byte_vector(bytes: Vec<u8>) -> Self;
    // TODO from_byte_buffer (Bytes) ? may not be needed.
}

struct CountingWriter {
    count: usize,
}
impl Writer for CountingWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<(), EncodeError> {
        self.count += bytes.len();
        Ok(())
    }
}

struct BytesBufReader(Bytes);

impl Reader for BytesBufReader {
    fn read(&mut self, bytes: &mut [u8]) -> Result<(), DecodeError> {
        self.0.copy_to_slice(bytes);
        Ok(())
    }
}

/// Implementation of AsBytes for all types that are binpackable.
impl<T: Encode + Decode + Sized> AsByteBuffer for T {
    fn size_bytes(&self) -> usize
    where
        Self: Encode,
    {
        // For now be careful with this as we have to bincode the whole thing in order to calculate
        // this. In the long run with a zero-copy implementation we can just return the size of the
        // underlying bytes.
        let mut cw = CountingWriter { count: 0 };
        bincode::encode_into_writer(self, &mut cw, *BINCODE_CONFIG)
            .expect("bincode to bytes for counting size");
        cw.count
    }

    fn as_byte_buffer(&self) -> Bytes
    where
        Self: Sized + Encode,
    {
        let v = bincode::encode_to_vec(self, *BINCODE_CONFIG).expect("bincode to bytes");
        Bytes::from(v)
    }

    fn from_byte_vector(bytes: Vec<u8>) -> Self
    where
        Self: Sized + Decode,
    {
        bincode::decode_from_slice(&bytes[..], *BINCODE_CONFIG)
            .expect("bincode from bytes")
            .0
    }
}
