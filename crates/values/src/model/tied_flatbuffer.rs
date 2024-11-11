// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use crate::DecodingError;
use bytes::Bytes;
use flatbuffers::{Follow, Verifiable};

/// A utility for tying the root of a flatbuffer to the data it was created from.
/// Done to avoid having to request the root (and re-validate) every time we need it.
pub(crate) struct TiedFlatBuffer<T>
where
    T: 'static + Follow<'static, Inner = T> + Verifiable,
{
    data: Bytes,
    root: T,
}

impl<T: 'static + Follow<'static, Inner = T> + Verifiable> TiedFlatBuffer<T> {
    pub fn open(data: Bytes) -> Result<Self, DecodingError> {
        // Have to use some unsafe here to trick the compiler into knowing that the lifetime of
        // the root is the same as the data.
        // This is safe because the root is a reference to the data, and the data is owned by the
        // TiedFlatBuffer.
        let root = unsafe {
            let data_ptr = data.as_ref() as *const [u8] as *const u8;
            let data = std::slice::from_raw_parts(data_ptr, data.len());
            flatbuffers::root::<T>(data).map_err(|e| {
                DecodingError::CouldNotDecode(format!("Could not decode flatbuffer: {:?}", e))
            })?
        };
        Ok(Self {
            data,
            root,
        })
    }

    pub fn root(&self) -> &T {
        &self.root
    }

    pub fn data(&self) -> &Bytes {
        &self.data
    }
}

/// A macro to generate a struct that wraps a TiedFlatBuffer and implements conveniences like Clone,
/// AsByteBuffer, a getter for the flatbuffer, a constructor from a FlatBufferBuilder, and a
/// validator for the data version.
#[macro_export]
macro_rules! tied_flatbuffer {
    ($holder:ident, $fb:ty) => {
        pub struct $holder(TiedFlatBuffer<$fb>);

        impl $holder {
            fn build(builder: FlatBufferBuilder) -> Self {
                let (vec, start) = builder.collapse();
                let b = Bytes::from(vec).slice(start..);
                Self(TiedFlatBuffer::open(b).expect("decoding error on build"))
            }

            fn validate_data_version(&self) -> Result<(), DecodingError> {
                let version = self.0.root().data_version();
                // TODO: we're going to have to implement a versioning scheme for the data layout
                //   likely by adding semver support.
                //   parsing the semver on every decode is going to be expensive, so we may want to
                //   hold pre-parsed versions in a static somewhere.
                if version != DATA_LAYOUT_VERSION {
                    return Err(DecodingError::CouldNotDecode(format!(
                        "Data version mismatch for entity '{}': expected {}, got {}",
                        stringify!($holder),
                        DATA_LAYOUT_VERSION,
                        version
                    )));
                }
                Ok(())
            }

            pub fn get_flatbuffer(&self) -> &$fb {
                self.0.root()
            }
        }

        impl AsByteBuffer for $holder {
            fn size_bytes(&self) -> usize {
                self.0.data().len()
            }

            fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(
                &self,
                mut f: F,
            ) -> Result<R, EncodingError> {
                Ok(f(self.0.data().as_ref()))
            }

            fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
                Ok(self.0.data().as_ref().to_vec())
            }

            fn from_bytes(bytes: Bytes) -> Result<Self, DecodingError> {
                let result = Self(TiedFlatBuffer::open(bytes)?);
                result.validate_data_version()?;
                Ok(result)
            }

            fn as_bytes(&self) -> Result<Bytes, EncodingError> {
                Ok(self.0.data().clone())
            }
        }

        impl Clone for $holder {
            fn clone(&self) -> Self {
                let bytes = self.0.data().clone();
                Self(TiedFlatBuffer::open(bytes).expect("decoding error on clone"))
            }
        }
    };
}
