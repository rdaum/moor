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

use crate::tx::{Error, Provider, Timestamp};
use bytes::Bytes;
use fjall::UserValue;
use moor_values::AsByteBuffer;
use std::marker::PhantomData;

/// A provider that fills the DB cache from a Fjall partition.
pub(crate) struct FjallProvider<Domain, Codomain>
where
    Domain: Clone + Eq + PartialEq + AsByteBuffer,
    Codomain: Clone + Eq + PartialEq + AsByteBuffer,
{
    fjall_partition: fjall::PartitionHandle,
    _phantom_data: PhantomData<(Domain, Codomain)>,
}

fn decode<Codomain>(user_value: UserValue) -> Result<(Timestamp, Codomain), Error>
where
    Codomain: AsByteBuffer,
{
    let result: Bytes = user_value.into();
    let ts = Timestamp(u64::from_le_bytes(result[0..8].try_into().unwrap()));
    let codomain = Codomain::from_bytes(result.slice(8..)).map_err(|_| Error::EncodingFailure)?;
    Ok((ts, codomain))
}

fn encode<Codomain>(ts: Timestamp, codomain: Codomain) -> Result<UserValue, Error>
where
    Codomain: AsByteBuffer,
{
    let as_bytes = codomain.as_bytes().map_err(|_| Error::EncodingFailure)?;
    let mut result = Vec::with_capacity(8 + as_bytes.len());
    result.extend_from_slice(&ts.0.to_le_bytes());
    result.extend_from_slice(&as_bytes);
    Ok(UserValue::from(Bytes::from(result)))
}

impl<Domain, Codomain> FjallProvider<Domain, Codomain>
where
    Domain: Clone + Eq + PartialEq + AsByteBuffer,
    Codomain: Clone + Eq + PartialEq + AsByteBuffer,
{
    pub fn new(fjall_partition: fjall::PartitionHandle) -> Self {
        Self {
            fjall_partition,
            _phantom_data: PhantomData,
        }
    }
}

impl<Domain, Codomain> Provider<Domain, Codomain> for FjallProvider<Domain, Codomain>
where
    Domain: Clone + Eq + PartialEq + AsByteBuffer,
    Codomain: Clone + Eq + PartialEq + AsByteBuffer,
{
    fn get(&self, domain: &Domain) -> Result<Option<(Timestamp, Codomain)>, Error> {
        let key = domain.as_bytes().map_err(|_| Error::EncodingFailure)?;
        let Some(result) = self
            .fjall_partition
            .get(key)
            .map_err(|_| Error::RetrievalFailure)?
        else {
            return Ok(None);
        };
        let (ts, codomain) = decode::<Codomain>(result)?;
        Ok(Some((ts, codomain)))
    }

    fn put(&self, timestamp: Timestamp, domain: Domain, codomain: Codomain) -> Result<(), Error> {
        let key = domain.as_bytes().map_err(|_| Error::EncodingFailure)?;
        let value = encode::<Codomain>(timestamp, codomain)?;
        self.fjall_partition
            .insert(key, value)
            .map_err(|_| Error::StorageFailure)?;
        Ok(())
    }

    fn del(&self, _timestamp: Timestamp, domain: &Domain) -> Result<(), Error> {
        let key = domain.as_bytes().map_err(|_| Error::EncodingFailure)?;
        self.fjall_partition
            .remove(key)
            .map_err(|_| Error::StorageFailure)?;
        Ok(())
    }

    fn scan<F>(&self, predicate: &F) -> Result<Vec<(Timestamp, Domain, Codomain)>, Error>
    where
        F: Fn(&Domain, &Codomain) -> bool,
    {
        let mut result = Vec::new();
        for entry in self.fjall_partition.iter() {
            let (key, value) = entry.map_err(|_| Error::RetrievalFailure)?;
            let domain = Domain::from_bytes(key.into()).map_err(|_| Error::EncodingFailure)?;
            let (ts, codomain) = decode::<Codomain>(value)?;
            if predicate(&domain, &codomain) {
                result.push((ts, domain, codomain));
            }
        }
        Ok(result)
    }
}
