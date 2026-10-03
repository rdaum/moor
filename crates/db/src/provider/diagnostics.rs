// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

//! Backend diagnostics use byte counts and durations, never stored values or credentials.

/// One PostgreSQL writer's cumulative counters and currently retained payloads.
/// Durations are nanoseconds unless the field explicitly says microseconds.
#[derive(Clone, Copy, Debug, Default)]
pub struct PostgresPersistenceStats {
    pub active_exports: u64,
    pub oldest_export_micros: u64,
    pub export_limit: u64,
    pub encoding_calls: u64,
    pub encoding_failures: u64,
    pub encoding_ns: u64,
    pub append_validation_cache_hits: u64,
    pub append_validation_cache_misses: u64,
    pub encoded_bytes: u64,
    pub sql_application_ns: u64,
    pub sql_commit_ns: u64,
    pub fence_calls: u64,
    pub fence_ns: u64,
    pub groups: u64,
    pub group_commits: u64,
    pub group_payload_bytes: u64,
    pub group_sql_statements: u64,
    pub last_group_commits: u64,
    pub last_group_payload_bytes: u64,
    pub last_group_sql_statements: u64,
    pub last_group_first: u64,
    pub last_group_last: u64,
    pub group_end_reasons: [u64; 7],
    pub recovery_attempts: u64,
    pub recovery_ns: u64,
    pub recovery_first: u64,
    pub recovery_last: u64,
    /// Validated commits waiting for publication, including attempts that can still conflict.
    pub prepared_commits: u64,
    /// Published commits whose encoded data is still retained by the writer.
    pub unapplied_commits: u64,
    /// Encoded JSON payload, excluding allocation overhead and future record sequence fields.
    pub retained_encoded_bytes: u64,
    /// Logical size of retained final append values; shared allocations can be counted repeatedly.
    pub retained_append_value_bytes: u64,
    pub oldest_unapplied_micros: u64,
}

/// Reason a SQL group ended. Array indices in `group_end_reasons` follow this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum PostgresGroupEnd {
    Available,
    CommitLimit,
    PayloadLimit,
    OperationLimit,
    AgeLimit,
    Fence,
    RollupExpansion,
}

impl PostgresPersistenceStats {
    pub(crate) fn operator_metrics(&self) -> Vec<(&'static str, u64)> {
        let mut metrics = vec![
            ("persistence_postgres_active_exports", self.active_exports),
            (
                "persistence_postgres_oldest_export_micros",
                self.oldest_export_micros,
            ),
            ("persistence_postgres_export_limit", self.export_limit),
            (
                "persistence_postgres_prepared_commits",
                self.prepared_commits,
            ),
            (
                "persistence_postgres_unapplied_commits",
                self.unapplied_commits,
            ),
            (
                "persistence_postgres_retained_encoded_bytes",
                self.retained_encoded_bytes,
            ),
            (
                "persistence_postgres_retained_append_value_bytes",
                self.retained_append_value_bytes,
            ),
            (
                "persistence_postgres_oldest_unapplied_micros",
                self.oldest_unapplied_micros,
            ),
            ("persistence_postgres_encoding_calls", self.encoding_calls),
            (
                "persistence_postgres_append_validation_cache_hits",
                self.append_validation_cache_hits,
            ),
            (
                "persistence_postgres_append_validation_cache_misses",
                self.append_validation_cache_misses,
            ),
            (
                "persistence_postgres_encoding_failures",
                self.encoding_failures,
            ),
            ("persistence_postgres_encoded_bytes", self.encoded_bytes),
            ("persistence_postgres_encoding_ns", self.encoding_ns),
            (
                "persistence_postgres_sql_application_ns",
                self.sql_application_ns,
            ),
            ("persistence_postgres_sql_commit_ns", self.sql_commit_ns),
            ("persistence_postgres_fence_calls", self.fence_calls),
            ("persistence_postgres_fence_ns", self.fence_ns),
            ("persistence_postgres_groups", self.groups),
            ("persistence_postgres_group_commits", self.group_commits),
            (
                "persistence_postgres_group_payload_bytes",
                self.group_payload_bytes,
            ),
            (
                "persistence_postgres_group_sql_statements",
                self.group_sql_statements,
            ),
            (
                "persistence_postgres_last_group_commits",
                self.last_group_commits,
            ),
            (
                "persistence_postgres_last_group_payload_bytes",
                self.last_group_payload_bytes,
            ),
            (
                "persistence_postgres_last_group_sql_statements",
                self.last_group_sql_statements,
            ),
            (
                "persistence_postgres_last_group_first",
                self.last_group_first,
            ),
            ("persistence_postgres_last_group_last", self.last_group_last),
            (
                "persistence_postgres_recovery_attempts",
                self.recovery_attempts,
            ),
            ("persistence_postgres_recovery_ns", self.recovery_ns),
            ("persistence_postgres_recovery_first", self.recovery_first),
            ("persistence_postgres_recovery_last", self.recovery_last),
        ];
        for (name, count) in [
            "persistence_postgres_group_end_available",
            "persistence_postgres_group_end_commit_limit",
            "persistence_postgres_group_end_payload_limit",
            "persistence_postgres_group_end_operation_limit",
            "persistence_postgres_group_end_age_limit",
            "persistence_postgres_group_end_fence",
            "persistence_postgres_group_end_rollup_expansion",
        ]
        .into_iter()
        .zip(self.group_end_reasons)
        {
            metrics.push((name, count));
        }
        metrics
    }
}
