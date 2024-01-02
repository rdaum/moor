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

use binary_layout::define_layout;
use std::sync::Arc;

use crate::tuplebox::RelationId;
use moor_values::util::slice_ref::SliceRef;

use crate::tuplebox::tuples::TupleRef;
use crate::tuplebox::tuples::{SlotBox, TupleId};

define_layout!(tuple_header, LittleEndian, {
    ts: u64,
    domain_size: u32,
    codomain_size: u32,
});

#[derive(Clone)]
pub struct Tuple(TupleRef);

impl Tuple {
    /// Allocate the given tuple in a slotbox.
    pub fn allocate(
        relation_id: RelationId,
        sb: Arc<SlotBox>,
        ts: u64,
        domain: &[u8],
        codomain: &[u8],
    ) -> TupleRef {
        let total_size = tuple_header::SIZE.unwrap() + domain.len() + codomain.len();
        let tuple_id = sb
            .allocate(total_size, relation_id, None)
            .expect("Failed to allocate tuple");
        sb.update_with(tuple_id, |mut buffer| {
            {
                let mut header = tuple_header::View::new(buffer.as_mut().get_mut());
                header.ts_mut().write(ts);
                header.domain_size_mut().write(domain.len() as u32);
                header.codomain_size_mut().write(codomain.len() as u32);
            }
            let start_pos = tuple_header::SIZE.unwrap();
            buffer[start_pos..start_pos + domain.len()].copy_from_slice(domain);
            buffer[start_pos + domain.len()..].copy_from_slice(codomain);
        })
        .unwrap();
        TupleRef::new(sb.clone(), tuple_id)
    }

    fn buffer(&self) -> SliceRef {
        SliceRef::from_byte_source(Box::new(self.0.clone()))
    }

    pub fn update_timestamp(&self, relation_id: RelationId, sb: Arc<SlotBox>, ts: u64) {
        let mut buffer = self.buffer().as_slice().to_vec();
        let mut header = tuple_header::View::new(&mut buffer);
        header.ts_mut().write(ts);
        let id = self.0.id;
        let new_id = sb
            .update(relation_id, self.0.id, buffer.as_slice())
            .unwrap();
        assert_eq!(id, new_id);
    }

    pub fn from_tuple_id(sb: Arc<SlotBox>, tuple_id: TupleId) -> Self {
        Self(TupleRef::new(sb, tuple_id))
    }

    pub fn ts(&self) -> u64 {
        let buffer = self.buffer();
        tuple_header::View::new(buffer.as_slice()).ts().read()
    }

    pub fn domain(&self) -> SliceRef {
        let buffer = self.buffer();
        let domain_size = tuple_header::View::new(buffer.as_slice())
            .domain_size()
            .read();
        buffer
            .slice(tuple_header::SIZE.unwrap()..tuple_header::SIZE.unwrap() + domain_size as usize)
    }

    pub fn codomain(&self) -> SliceRef {
        let buffer = self.buffer();

        let domain_size = tuple_header::View::new(buffer.as_slice())
            .domain_size()
            .read() as usize;
        let codomain_size = tuple_header::View::new(buffer.as_slice())
            .codomain_size()
            .read() as usize;
        buffer.slice(
            tuple_header::SIZE.unwrap() + domain_size
                ..tuple_header::SIZE.unwrap() + domain_size + codomain_size,
        )
    }
}
