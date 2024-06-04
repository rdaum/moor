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

use crate::bindings::DataSource::Table;
use crate::bindings::FormatType::RawByte;
use crate::bindings::{CreateConfig, DataSource, FormatType, Session};
use std::fmt::Display;
use std::str::FromStr;
use strum::{EnumProperty, IntoEnumIterator};

pub trait WiredTigerRelation:
    Into<DataSource> + Display + Copy + IntoEnumIterator + EnumProperty
{
    fn get_secondary_index(&self) -> DataSource {
        assert!(self.has_secondary_index());
        DataSource::Index {
            table: self.to_string(),
            index_name: "codomain".to_string(),
            projection: Some(
                self.domain_columns()
                    .iter()
                    .map(|it| it.to_string())
                    .collect(),
            ),
        }
    }

    fn domain_columns(&self) -> Vec<&str> {
        if self.has_composite_domain() {
            vec!["domain_a", "domain_b"]
        } else {
            vec!["domain"]
        }
    }

    fn columns(&self) -> Vec<&str> {
        if self.has_composite_domain() {
            vec!["domain_a", "domain_b", "codomain"]
        } else {
            vec!["domain", "codomain"]
        }
    }

    fn domain_key_format(&self) -> Vec<FormatType> {
        if self.has_composite_domain() {
            let domain_a_size = self
                .get_str("Domain_A_Size")
                .expect("No size declared for Domain_A");
            let domain_a_size = usize::from_str(domain_a_size).unwrap();
            let domain_b_size = self
                .get_str("Domain_B_Size")
                .expect("No size declared for Domain_B");
            let domain_b_size = usize::from_str(domain_b_size).unwrap();
            vec![RawByte(Some(domain_a_size)), RawByte(Some(domain_b_size))]
        } else {
            vec![RawByte(None)]
        }
    }
    fn create_tables(tx: &Session) {
        for rel in Self::iter() {
            rel.create_table(tx);
        }
    }
    fn has_tables(session: &Session) -> bool {
        for rel in Self::iter() {
            if session.open_cursor(&rel.into(), None).is_err() {
                return false;
            }
        }
        true
    }

    #[allow(dead_code)]
    fn has_composite_domain(&self) -> bool {
        self.get_str("CompositeDomain")
            .map(|it| it == "true")
            .unwrap_or(false)
    }

    fn create_table(&self, session: &Session) {
        let table = Table(self.to_string());

        let columns = &self.columns();

        let key_format = &self.domain_key_format();
        let value_format = &[RawByte(None)];
        let options = CreateConfig::new()
            .columns(columns)
            .key_format(key_format)
            .value_format(value_format);

        session.create(&table, Some(options)).unwrap();

        // If there's a need for secondary index on the codomain, create it.
        if self.has_secondary_index() {
            let options = CreateConfig::new().columns(&["codomain"]);
            let index = DataSource::Index {
                table: self.to_string(),
                index_name: "codomain".to_string(),
                projection: None,
            };
            session.create(&index, Some(options)).unwrap();
        }
    }

    fn has_secondary_index(&self) -> bool {
        self.get_str("SecondaryIndexed")
            .map(|it| it == "true")
            .unwrap_or(false)
    }
}

impl<R: WiredTigerRelation> From<R> for DataSource {
    fn from(val: R) -> Self {
        Table(val.to_string())
    }
}
