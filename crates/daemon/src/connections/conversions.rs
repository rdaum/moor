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

//! Conversion functions between in-memory and FlatBuffers representations

use crate::connections::{ConnectionRecord, ConnectionsRecords, connections_generated};
use eyre::Error;
use moor_schema::convert as convert_schema;
use planus::{ReadAsRoot, WriteAsOffset};
use std::time::{Duration, UNIX_EPOCH};

/// Convert in-memory ConnectionRecord to FlatBuffers struct representation
pub fn connection_record_to_fb(
    record: &ConnectionRecord,
) -> Result<connections_generated::moor_connections::ConnectionRecord, Error> {
    // Convert acceptable_content_types to FlatBuffer Symbols
    let content_types: Vec<_> = record
        .acceptable_content_types
        .iter()
        .map(|symbol| connections_generated::moor_common::Symbol {
            value: symbol.as_string(),
        })
        .collect();

    // Convert client_attributes to FlatBuffer ClientAttribute pairs
    let attributes: Result<Vec<_>, Error> = record
        .client_attributes
        .iter()
        .map(|(key, value)| -> Result<_, Error> {
            // Convert Symbol to connections_generated Symbol
            let fb_key = connections_generated::moor_common::Symbol {
                value: key.as_string(),
            };

            // Convert Var via serialization: Var -> moor_schema::Var -> bytes -> connections_generated::Var
            let schema_var = convert_schema::var_to_db_flatbuffer(value)?;
            let var_bytes = {
                let mut builder = planus::Builder::new();
                let offset = planus::WriteAsOffset::prepare(&schema_var, &mut builder);
                builder.finish(offset, None);
                builder.as_slice().to_vec()
            };
            let fb_value: connections_generated::moor_var::Var =
                connections_generated::moor_var::VarRef::read_as_root(&var_bytes)?.try_into()?;

            Ok(connections_generated::moor_connections::ClientAttribute {
                key: Box::new(fb_key),
                value: Box::new(fb_value),
            })
        })
        .collect();
    let attributes = attributes?;

    // Convert SystemTime to secs/nanos
    let connected = record
        .connected_time
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let last_activity = record
        .last_activity
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let last_ping = record
        .last_ping
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    Ok(connections_generated::moor_connections::ConnectionRecord {
        client_id_high: (record.client_id >> 64) as u64,
        client_id_low: record.client_id as u64,
        connected_secs: connected.as_secs(),
        connected_nanos: connected.subsec_nanos(),
        last_activity_secs: last_activity.as_secs(),
        last_activity_nanos: last_activity.subsec_nanos(),
        last_ping_secs: last_ping.as_secs(),
        last_ping_nanos: last_ping.subsec_nanos(),
        hostname: record.hostname.clone(),
        local_port: record.local_port,
        remote_port: record.remote_port,
        acceptable_content_types: content_types,
        client_attributes: attributes,
    })
}

/// Convert FlatBuffers ConnectionRecordRef to in-memory representation
pub fn connection_record_from_fb(
    fb: &connections_generated::moor_connections::ConnectionRecordRef,
) -> Result<ConnectionRecord, Error> {
    let client_id = ((fb.client_id_high()? as u128) << 64) | (fb.client_id_low()? as u128);

    let connected_time = UNIX_EPOCH + Duration::new(fb.connected_secs()?, fb.connected_nanos()?);
    let last_activity =
        UNIX_EPOCH + Duration::new(fb.last_activity_secs()?, fb.last_activity_nanos()?);
    let last_ping = UNIX_EPOCH + Duration::new(fb.last_ping_secs()?, fb.last_ping_nanos()?);

    let hostname = fb.hostname()?.to_string();

    // Deserialize acceptable_content_types
    let mut acceptable_content_types = Vec::new();
    for symbol_ref in fb.acceptable_content_types()? {
        let symbol_str = symbol_ref?.value()?;
        acceptable_content_types.push(moor_var::Symbol::mk(symbol_str));
    }

    // Deserialize client_attributes
    let mut client_attributes = std::collections::HashMap::new();
    for attr_result in fb.client_attributes()? {
        let attr = attr_result?;

        // Extract Symbol from connections_generated type
        let key_str = attr.key()?.value()?;
        let key = moor_var::Symbol::mk(key_str);

        // Convert Var via serialization: connections_generated::Var -> bytes -> moor_schema::Var -> Var
        let value_ref = attr.value()?;
        let value_bytes = {
            let connections_var_owned = connections_generated::moor_var::Var::try_from(value_ref)?;
            let mut builder = planus::Builder::new();
            let offset = planus::WriteAsOffset::prepare(&connections_var_owned, &mut builder);
            builder.finish(offset, None);
            builder.as_slice().to_vec()
        };
        let schema_var_ref = moor_schema::var::VarRef::read_as_root(&value_bytes)?;
        let value = convert_schema::var_from_ref(schema_var_ref)
            .map_err(|e| eyre::eyre!("Failed to convert value var: {}", e))?;

        client_attributes.insert(key, value);
    }

    Ok(ConnectionRecord {
        client_id,
        connected_time,
        last_activity,
        last_ping,
        hostname,
        local_port: fb.local_port()?,
        remote_port: fb.remote_port()?,
        acceptable_content_types,
        client_attributes,
    })
}

/// Convert in-memory ConnectionsRecords to FlatBuffers bytes
pub fn connections_records_to_bytes(records: &ConnectionsRecords) -> Result<Vec<u8>, Error> {
    let mut fb_connections = Vec::new();
    for record in &records.connections {
        fb_connections.push(connection_record_to_fb(record)?);
    }

    let fb_records = connections_generated::moor_connections::ConnectionsRecords {
        connections: fb_connections,
    };

    let mut builder = planus::Builder::new();
    let offset = WriteAsOffset::prepare(&fb_records, &mut builder);
    builder.finish(offset, None);
    Ok(builder.as_slice().to_vec())
}

/// Convert FlatBuffers bytes to in-memory ConnectionsRecords
pub fn connections_records_from_bytes(bytes: &[u8]) -> Result<ConnectionsRecords, Error> {
    let fb = connections_generated::moor_connections::ConnectionsRecordsRef::read_as_root(bytes)?;

    let mut connections = Vec::new();
    for fb_record in fb.connections()? {
        let fb_record = fb_record?;
        connections.push(connection_record_from_fb(&fb_record)?);
    }

    Ok(ConnectionsRecords { connections })
}
