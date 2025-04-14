// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use crate::tx_management::{Error, Provider, Timestamp};
use byteview::ByteView;
use crossbeam_channel::Sender;
use fjall::UserValue;
use moor_var::AsByteBuffer;
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread::JoinHandle;
use tracing::error;

/// A backing persistence provider that fills the DB cache from a Fjall partition.
pub(crate) struct FjallProvider<Domain, Codomain>
where
    Domain: Clone + Eq + PartialEq + AsByteBuffer,
    Codomain: Clone + Eq + PartialEq + AsByteBuffer,
{
    fjall_partition: fjall::PartitionHandle,
    insert_tx: Sender<(Timestamp, Domain, Codomain)>,
    delete_tx: Sender<Domain>,
    kill_switch: Arc<AtomicBool>,
    _phantom_data: PhantomData<(Domain, Codomain)>,
    jh: Option<JoinHandle<()>>,
}

fn decode<Codomain>(user_value: UserValue) -> Result<(Timestamp, Codomain), Error>
where
    Codomain: AsByteBuffer,
{
    let result: ByteView = user_value.into();
    let ts = Timestamp(u64::from_le_bytes(result[0..8].try_into().unwrap()));
    let codomain = Codomain::from_bytes(result.slice(8..)).map_err(|_| Error::EncodingFailure)?;
    Ok((ts, codomain))
}

fn encode<Codomain>(ts: Timestamp, codomain: &Codomain) -> Result<UserValue, Error>
where
    Codomain: AsByteBuffer,
{
    let as_bytes = codomain.as_bytes().map_err(|_| Error::EncodingFailure)?;
    let mut result = Vec::with_capacity(8 + as_bytes.len());
    result.extend_from_slice(&ts.0.to_le_bytes());
    result.extend_from_slice(&as_bytes);
    Ok(UserValue::from(ByteView::from(result)))
}

impl<Domain, Codomain> FjallProvider<Domain, Codomain>
where
    Domain: Clone + Eq + PartialEq + AsByteBuffer + Send + 'static,
    Codomain: Clone + Eq + PartialEq + AsByteBuffer + Send + 'static,
{
    pub fn new(fjall_partition: fjall::PartitionHandle) -> Self {
        let kill_switch = Arc::new(AtomicBool::new(false));
        let (insert_tx, insert_rx) =
            crossbeam_channel::unbounded::<(Timestamp, Domain, Codomain)>();
        let (delete_tx, delete_rx) = crossbeam_channel::unbounded::<Domain>();

        let fj = fjall_partition.clone();
        let ks = kill_switch.clone();
        let jh = std::thread::spawn(move || {
            loop {
                if ks.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }
                if let Ok((ts, domain, codomain)) = insert_rx.try_recv() {
                    let Ok(key) = domain.as_bytes().map_err(|_| {
                        error!("failed to encode domain to database");
                    }) else {
                        continue;
                    };
                    let Ok(value) = encode::<Codomain>(ts, &codomain) else {
                        error!("failed to encode codomain to database");
                        continue;
                    };
                    fjall_partition
                        .insert(key, value)
                        .map_err(|e| {
                            error!("failed to insert into database: {}", e);
                        })
                        .ok();
                }

                if let Ok(domain) = delete_rx.try_recv() {
                    let Ok(key) = domain.as_bytes().map_err(|_| {
                        error!("failed to encode domain to database for deletion");
                    }) else {
                        continue;
                    };
                    fjall_partition
                        .remove(key)
                        .map_err(|e| {
                            error!("failed to delete from database: {}", e);
                        })
                        .ok();
                }
                std::thread::sleep(std::time::Duration::from_micros(1));
            }
        });
        Self {
            fjall_partition: fj,
            insert_tx,
            delete_tx,
            _phantom_data: PhantomData,
            kill_switch,
            jh: Some(jh),
        }
    }
}

impl<Domain, Codomain> Provider<Domain, Codomain> for FjallProvider<Domain, Codomain>
where
    Domain: Clone + Eq + PartialEq + AsByteBuffer,
    Codomain: Clone + Eq + PartialEq + AsByteBuffer,
{
    fn get(&self, domain: &Domain) -> Result<Option<(Timestamp, Codomain, usize)>, Error> {
        let key = domain.as_bytes().map_err(|_| Error::EncodingFailure)?;
        let key_len = key.len();
        let Some(result) = self
            .fjall_partition
            .get(key)
            .map_err(|e| Error::RetrievalFailure(e.to_string()))?
        else {
            return Ok(None);
        };
        let size = key_len + result.len();
        let (ts, codomain) = decode::<Codomain>(result)?;
        Ok(Some((ts, codomain, size)))
    }

    fn put(&self, timestamp: Timestamp, domain: &Domain, codomain: &Codomain) -> Result<(), Error> {
        if let Err(e) = self
            .insert_tx
            .send((timestamp, domain.clone(), codomain.clone()))
        {
            return Err(Error::StorageFailure(format!(
                "failed to insert into database: {}",
                e
            )));
        }
        Ok(())
    }

    fn del(&self, _timestamp: Timestamp, domain: &Domain) -> Result<(), Error> {
        if let Err(e) = self.delete_tx.send(domain.clone()) {
            return Err(Error::StorageFailure(format!(
                "failed to delete from database: {}",
                e
            )));
        };
        Ok(())
    }

    fn scan<F>(&self, predicate: &F) -> Result<Vec<(Timestamp, Domain, Codomain, usize)>, Error>
    where
        F: Fn(&Domain, &Codomain) -> bool,
    {
        let mut result = Vec::new();
        for entry in self.fjall_partition.iter() {
            let (key, value) = entry.map_err(|e| Error::RetrievalFailure(e.to_string()))?;
            let size = key.len() + value.len();
            let domain =
                Domain::from_bytes(key.clone().into()).map_err(|_| Error::EncodingFailure)?;
            let (ts, codomain) = decode::<Codomain>(value)?;
            if predicate(&domain, &codomain) {
                result.push((ts, domain, codomain, size));
            }
        }
        Ok(result)
    }

    fn stop(&self) -> Result<(), Error> {
        self.kill_switch
            .store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

impl<Domain, Codomain> Drop for FjallProvider<Domain, Codomain>
where
    Domain: Clone + Eq + PartialEq + AsByteBuffer,
    Codomain: Clone + Eq + PartialEq + AsByteBuffer,
{
    fn drop(&mut self) {
        self.stop().unwrap();
        if let Some(jh) = self.jh.take() {
            jh.join().unwrap();
        }
    }
}
