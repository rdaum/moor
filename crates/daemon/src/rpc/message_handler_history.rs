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

use crate::rpc::message_handler::RpcMessageHandler;
use moor_schema::{
    convert::{uuid_from_ref, uuid_to_flatbuffer_struct},
    rpc as moor_rpc,
};
use moor_var::Obj;
use rpc_common::RpcMessageError;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::debug;
use uuid::Uuid;

impl RpcMessageHandler {
    pub(crate) fn build_history_response(
        &self,
        player: Obj,
        history_recall_ref: moor_rpc::HistoryRecallRef<'_>,
    ) -> Result<moor_rpc::HistoryResponse, RpcMessageError> {
        let (events, total_events_available, has_more_before) = match history_recall_ref
            .recall()
            .map_err(|_| RpcMessageError::InvalidRequest("Missing history recall".to_string()))?
        {
            moor_rpc::HistoryRecallUnionRef::HistoryRecallSinceEvent(since) => {
                let event_id_ref = since
                    .event_id()
                    .map_err(|_| RpcMessageError::InvalidRequest("Missing event_id".to_string()))?;
                let since_id = uuid_from_ref(event_id_ref)
                    .map_err(|e| RpcMessageError::InvalidRequest(e.to_string()))?;
                let limit_val = since
                    .limit()
                    .map_err(|_| RpcMessageError::InvalidRequest("Missing limit".to_string()))?;
                let limit = if limit_val == 0 {
                    None
                } else {
                    Some(limit_val as usize)
                };

                let all_events = self
                    .event_log
                    .events_for_player_since(player, Some(since_id));
                let total_available = all_events.len();
                let has_more = limit.is_some_and(|l| total_available > l);
                let events = if let Some(limit) = limit {
                    all_events.into_iter().take(limit).collect()
                } else {
                    all_events
                };
                (events, total_available, has_more)
            }
            moor_rpc::HistoryRecallUnionRef::HistoryRecallUntilEvent(until) => {
                let event_id_ref = until
                    .event_id()
                    .map_err(|_| RpcMessageError::InvalidRequest("Missing event_id".to_string()))?;
                let until_id = uuid_from_ref(event_id_ref)
                    .map_err(|e| RpcMessageError::InvalidRequest(e.to_string()))?;
                let limit_val = until
                    .limit()
                    .map_err(|_| RpcMessageError::InvalidRequest("Missing limit".to_string()))?;
                let limit = if limit_val == 0 {
                    None
                } else {
                    Some(limit_val as usize)
                };

                let all_events = self
                    .event_log
                    .events_for_player_until(player, Some(until_id));
                let total_available = all_events.len();
                let has_more = limit.is_some_and(|l| total_available > l);
                let events = if let Some(limit) = limit {
                    // For UntilEvent, we want the MOST RECENT events before the boundary, not the oldest
                    // So take from the end of the chronologically sorted list
                    let len = all_events.len();
                    if len > limit {
                        all_events.into_iter().skip(len - limit).collect()
                    } else {
                        all_events
                    }
                } else {
                    all_events
                };
                (events, total_available, has_more)
            }
            moor_rpc::HistoryRecallUnionRef::HistoryRecallSinceSeconds(since_seconds) => {
                let seconds_ago = since_seconds.seconds_ago().map_err(|_| {
                    RpcMessageError::InvalidRequest("Missing seconds_ago".to_string())
                })?;
                let limit_val = since_seconds
                    .limit()
                    .map_err(|_| RpcMessageError::InvalidRequest("Missing limit".to_string()))?;
                let limit = if limit_val == 0 {
                    None
                } else {
                    Some(limit_val as usize)
                };

                let all_events = self
                    .event_log
                    .events_for_player_since_seconds(player, seconds_ago);
                let total_available = all_events.len();
                let has_more = limit.is_some_and(|l| total_available > l);
                let events = if let Some(limit) = limit {
                    // For SinceSeconds, we want the MOST RECENT events, not the oldest
                    // So take from the end of the chronologically sorted list
                    let len = all_events.len();
                    if len > limit {
                        all_events.into_iter().skip(len - limit).collect()
                    } else {
                        all_events
                    }
                } else {
                    all_events
                };
                (events, total_available, has_more)
            }
            moor_rpc::HistoryRecallUnionRef::HistoryRecallNone(_) => (Vec::new(), 0, false),
        };

        // Calculate metadata
        let (earliest_time, latest_time) = if events.is_empty() {
            (SystemTime::now(), SystemTime::now())
        } else {
            (
                UNIX_EPOCH + Duration::from_nanos(events.first().unwrap().timestamp),
                UNIX_EPOCH + Duration::from_nanos(events.last().unwrap().timestamp),
            )
        };

        debug!(
            "Built history response with {} events for player {} (total available: {}, has more: {}, time range: {:?} to {:?})",
            events.len(),
            player,
            total_events_available,
            has_more_before,
            earliest_time,
            latest_time
        );

        // Find actual earliest and latest event IDs from the returned events
        let (earliest_event_id, latest_event_id) = if events.is_empty() {
            (None, None)
        } else {
            let mut event_ids: Vec<_> = events
                .iter()
                .map(|e| {
                    // Convert FlatBuffer UUID to Uuid
                    let uuid_bytes = e.event_id.data.as_slice();
                    if uuid_bytes.len() == 16 {
                        let mut bytes = [0u8; 16];
                        bytes.copy_from_slice(uuid_bytes);
                        Uuid::from_bytes(bytes)
                    } else {
                        Uuid::nil()
                    }
                })
                .collect();
            event_ids.sort(); // UUIDs sort chronologically
            (Some(event_ids[0]), Some(event_ids[event_ids.len() - 1]))
        };

        // Convert to FlatBuffer structs
        let fb_events: Vec<_> = events
            .iter()
            .map(|logged_event| {
                // LoggedNarrativeEvent now stores encrypted blobs
                moor_rpc::HistoricalNarrativeEvent {
                    event_id: logged_event.event_id.clone(),
                    timestamp: logged_event.timestamp,
                    player: logged_event.player.clone(),
                    is_historical: true,
                    encrypted_blob: logged_event.encrypted_blob.clone(),
                }
            })
            .collect();

        let time_range_start = earliest_time
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let time_range_end = latest_time
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        Ok(moor_rpc::HistoryResponse {
            events: fb_events,
            time_range_start,
            time_range_end,
            total_events: total_events_available as u64,
            has_more_before,
            earliest_event_id: earliest_event_id.map(|id| Box::new(uuid_to_flatbuffer_struct(&id))),
            latest_event_id: latest_event_id.map(|id| Box::new(uuid_to_flatbuffer_struct(&id))),
        })
    }
}
