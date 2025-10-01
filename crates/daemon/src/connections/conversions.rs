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
use moor_var::BINCODE_CONFIG;
use planus::{ReadAsRoot, WriteAsOffset};
use std::time::{Duration, UNIX_EPOCH};

/// Convert in-memory ConnectionRecord to FlatBuffers struct representation
pub fn connection_record_to_fb(
    record: &ConnectionRecord,
) -> Result<connections_generated::moor_connections::ConnectionRecord, Error> {
    // Serialize acceptable_content_types as ByteArrays
    let mut content_types = Vec::new();
    for symbol in &record.acceptable_content_types {
        let bytes = bincode::encode_to_vec(symbol, *BINCODE_CONFIG)?;
        content_types.push(connections_generated::moor_connections::ByteArray { data: bytes });
    }

    // Serialize client_attributes as ClientAttribute pairs
    let mut attributes = Vec::new();
    for (key, value) in &record.client_attributes {
        let key_bytes = bincode::encode_to_vec(key, *BINCODE_CONFIG)?;
        let value_bytes = bincode::encode_to_vec(value, *BINCODE_CONFIG)?;

        attributes.push(connections_generated::moor_connections::ClientAttribute {
            key: Box::new(connections_generated::moor_connections::ByteArray { data: key_bytes }),
            value: Box::new(connections_generated::moor_connections::ByteArray {
                data: value_bytes,
            }),
        });
    }

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
    for byte_array_result in fb.acceptable_content_types()? {
        let byte_array = byte_array_result?;
        let data = byte_array.data()?;
        let (symbol, _) = bincode::decode_from_slice(data, *BINCODE_CONFIG)?;
        acceptable_content_types.push(symbol);
    }

    // Deserialize client_attributes
    let mut client_attributes = std::collections::HashMap::new();
    for attr_result in fb.client_attributes()? {
        let attr = attr_result?;
        let key_data = attr.key()?.data()?;
        let (key, _) = bincode::decode_from_slice(key_data, *BINCODE_CONFIG)?;

        let value_data = attr.value()?.data()?;
        let (value, _) = bincode::decode_from_slice(value_data, *BINCODE_CONFIG)?;

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
