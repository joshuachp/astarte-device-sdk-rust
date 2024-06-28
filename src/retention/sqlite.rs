// This file is part of Astarte.
//
// Copyright 2024 SECO Mind Srl
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// SPDX-License-Identifier: Apache-2.0

//! Retention implemented using an SQLite database.

use std::{
    array::TryFromSliceError,
    borrow::Cow,
    fmt::Display,
    time::{Duration, SystemTime, SystemTimeError, UNIX_EPOCH},
};

use futures::TryStreamExt;
use sqlx::{query_file, Sqlite, Transaction};
use tracing::error;

use crate::{error::Report, interface::Reliability, store::SqliteStore};

/// Error returned by the retention.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SqliteRetentionError {
    /// Couldn't get all the packets.
    #[error("couldn't get all the packets")]
    AllPackets(#[source] sqlx::Error),
    /// Couldn't convert timestamp bytes
    #[error("couldn't convert timestamp bytes")]
    Timestamp(#[source] TryFromSliceError),
    /// Reliability must not be at most once (0)
    #[error("invalid reliability ({0})")]
    Reliability(u8),
    /// Couldn't store the mapping
    #[error("couldn't store mapping {topic}")]
    StoreMapping {
        /// The source of the error
        #[source]
        backtrace: sqlx::Error,
        /// Topic of the mapping
        topic: String,
    },
    /// Couldn't store the publish
    #[error("couldn't store publish for {path}")]
    StorePublish {
        /// The source of the error
        #[source]
        backtrace: sqlx::Error,
        /// Topic of the mapping
        path: String,
    },
    /// Couldn't fetch mapping
    #[error("couldn't fetch mapping {path}")]
    Mapping {
        /// The source of the error
        #[source]
        backtrace: sqlx::Error,
        /// Path of the packet
        path: String,
    },
    /// Couldn't fetch publish
    #[error("couldn't fetch publish {id}")]
    Publish {
        /// The source of the error
        #[source]
        backtrace: sqlx::Error,
        /// Id of the publish
        id: Id,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct MappingRetention<'a> {
    path: Cow<'a, str>,
    version_major: i32,
    reliability: Reliability,
    expiry: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct MappingPacket<'a> {
    id: Id,
    path: Cow<'a, str>,
    payload: Cow<'a, [u8]>,
}

/// Id of a packet
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id {
    timestamp: u128,
    counter: u32,
}

impl Id {
    pub(crate) fn from_row(timestamp: &[u8], counter: u32) -> Result<Self, TryFromSliceError> {
        let bytes = timestamp.try_into()?;
        let timestamp = u128::from_be_bytes(bytes);

        Ok(Self { timestamp, counter })
    }

    pub(crate) fn timestamp_as_bytes(&self) -> [u8; 16] {
        self.timestamp.to_be_bytes()
    }
}

impl Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.timestamp, self.counter)
    }
}

pub(crate) struct Context {
    last_t: u128,
    counter: u32,
}

impl Context {
    fn new() -> Self {
        Self {
            last_t: 0,
            counter: 0,
        }
    }

    fn next(&mut self) -> Result<Id, SystemTimeError> {
        let timestamp = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(t) => t.as_millis(),
            Err(err) => {
                error!(error = %Report::new(&err), "untrasted system clock, time returned before unix epoch");

                return Err(err);
            }
        };

        // The clock is guarantied to be monotonic ascending.
        let counter = if timestamp - self.last_t > 0 {
            0
        } else {
            // If the ID overflows we insert the same primary key for the packet and throw an error.
            self.counter.saturating_add(1)
        };

        self.last_t = timestamp;
        self.counter = counter;

        Ok(Id { timestamp, counter })
    }
}

/// Data used to resend the stored packets.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct StoredPacket {
    id: Id,
    topic: String,
    payload: Vec<u8>,
    qos: Reliability,
}

/// Gets the [`Reliability`] from a stored [`u8`].
fn reliability_from_row(qos: u8) -> Option<Reliability> {
    match qos {
        1 => Some(Reliability::Guaranteed),
        2 => Some(Reliability::Unique),
        _ => None,
    }
}

// Implementation of utilities used in the retention
impl SqliteStore {
    /// Check if a [`PacketId`] already exists.
    async fn all_packets(
        &self,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<Vec<StoredPacket>, SqliteRetentionError> {
        query_file!("queries/retention/all_packets.sql")
            .fetch(&mut **transaction)
            .map_err(SqliteRetentionError::AllPackets)
            .and_then(|row| async move {
                let id = Id::from_row(&row.t_millis, row.counter)
                    .map_err(SqliteRetentionError::Timestamp)?;

                let qos = reliability_from_row(row.qos)
                    .ok_or(SqliteRetentionError::Reliability(row.qos))?;

                Ok(StoredPacket {
                    id,
                    topic: row.topic,
                    payload: row.payload,
                    qos,
                })
            })
            .try_collect()
            .await
    }

    async fn mapping_retention(
        &self,
        mapping: &MappingRetention<'_>,
    ) -> Result<(), SqliteRetentionError> {
        let qos: u8 = match mapping.reliability {
            Reliability::Unreliable => return Err(SqliteRetentionError::Reliability(0)),
            Reliability::Guaranteed => 1,
            Reliability::Unique => 2,
        };
        let exp: Option<i64> = mapping.expiry.and_then(|exp| {
            // If the conversion fails, since the u64 was to big for the i64, we will keep the
            // packet forever.
            exp.as_secs().try_into().ok()
        });

        query_file!(
            "queries/retention/store_mapping.sql",
            mapping.path,
            mapping.version_major,
            qos,
            exp
        )
        .execute(&self.db_conn)
        .await
        .map_err(|err| SqliteRetentionError::StoreMapping {
            backtrace: err,
            topic: mapping.path.to_string(),
        })?;

        Ok(())
    }

    async fn store_packet(&self, packet: &MappingPacket<'_>) -> Result<(), SqliteRetentionError> {
        let be_bytes = packet.id.timestamp.to_be_bytes();
        let timestamp = be_bytes.as_slice();
        let counter = packet.id.counter;

        let payload: &[u8] = &packet.payload;

        query_file!(
            "queries/retention/store_packet.sql",
            timestamp,
            counter,
            packet.path,
            payload,
        )
        .execute(&self.db_conn)
        .await
        .map_err(|err| SqliteRetentionError::StoreMapping {
            backtrace: err,
            topic: packet.path.to_string(),
        })?;

        Ok(())
    }

    async fn mapping(
        &self,
        path: &str,
    ) -> Result<Option<MappingRetention<'static>>, SqliteRetentionError> {
        let Some(row) = query_file!("queries/retention/mapping.sql", path)
            .fetch_optional(&self.db_conn)
            .await
            .map_err(|err| SqliteRetentionError::Mapping {
                backtrace: err,
                path: path.to_string(),
            })?
        else {
            return Ok(None);
        };

        let reliability =
            reliability_from_row(row.qos).ok_or(SqliteRetentionError::Reliability(row.qos))?;

        let expiry = row.expiry_sec.and_then(|exp| {
            // If the conversion fails, let's keep the packet forever.
            exp.try_into().ok().map(Duration::from_secs)
        });

        Ok(Some(MappingRetention {
            path: row.path.into(),
            version_major: row.major_version,
            reliability,
            expiry,
        }))
    }

    async fn packet(
        &self,
        id: &Id,
    ) -> Result<Option<MappingPacket<'static>>, SqliteRetentionError> {
        let timestamp = id.timestamp_as_bytes();
        let timestamp = timestamp.as_slice();

        let Some(row) = query_file!("queries/retention/packet.sql", timestamp, id.counter)
            .fetch_optional(&self.db_conn)
            .await
            .map_err(|err| SqliteRetentionError::Packet {
                backtrace: err,
                id: *id,
            })?
        else {
            return Ok(None);
        };

        let id = Id::from_row(&row.t_millis, row.counter).map_err(RetentionError::Timestamp)?;

        Ok(Some(MappingPacket {
            id,
            path: row.topic.into(),
            payload: row.payload.into(),
        }))
    }

    async fn packet_received(&self, id: &Id) -> Result<(), RetentionError> {
        let timestamp = id.timestamp_as_bytes();
        let timestamp = timestamp.as_slice();

        let res = query_file!(
            "queries/retention/packet_received.sql",
            timestamp,
            id.counter
        )
        .execute(&self.db_conn)
        .await
        .map_err(|err| RetentionError::PacketReceived {
            backtrace: err,
            id: *id,
        })?;

        let rows = res.rows_affected();
        if rows != 1 {
            return Err(RetentionError::DeleteReceived { id: *id, rows });
        }

        Ok(())
    }

    async fn resend_all(&self) -> Result<Vec<StoredPacket>, RetentionError> {
        let mut transaction = self
            .db_conn
            .begin()
            .await
            .map_err(RetentionError::Transaction)?;

        let packets = self.all_packets(&mut transaction).await?;

        transaction
            .commit()
            .await
            .map_err(RetentionError::ResendAll)?;

        Ok(packets)
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use super::*;

    #[test]
    fn should_be_unique_monotonic_ascending() {
        let mut ctx = Context::new();

        let mut prev = ctx.next().unwrap();
        for _i in 0..100 {
            let new = ctx.next().unwrap();

            assert!(new > prev);

            prev = new;
        }
    }

    #[tokio::test]
    async fn should_store_and_check_mapping() {
        let dir = tempfile::tempdir().unwrap();

        let store = SqliteStore::connect(dir.path()).await.unwrap();

        let mapping = MappingRetention {
            path: "realm/device_id/com.Foo/bar".into(),
            version_major: 1,
            qos: Reliability::Guaranteed,
            expiry: None,
        };
        store.store_mapping(&mapping).await.unwrap();

        let res = store.mapping(&mapping.path).await.unwrap().unwrap();

        assert_eq!(res, mapping);
    }

    #[tokio::test]
    async fn should_replace_mapping() {
        let dir = tempfile::tempdir().unwrap();

        let store = SqliteStore::connect(dir.path()).await.unwrap();

        let mut mapping = MappingRetention {
            path: "realm/device_id/com.Foo/bar".into(),
            version_major: 1,
            qos: Reliability::Guaranteed,
            expiry: None,
        };
        store.store_mapping(&mapping).await.unwrap();

        let res = store.mapping(&mapping.path).await.unwrap().unwrap();

        assert_eq!(res, mapping);

        mapping.version_major = 2;

        store.store_mapping(&mapping).await.unwrap();

        let res = store.mapping(&mapping.path).await.unwrap().unwrap();

        assert_eq!(res, mapping);
    }

    #[tokio::test]
    async fn expiry_too_big() {
        let dir = tempfile::tempdir().unwrap();

        let store = SqliteStore::connect(dir.path()).await.unwrap();

        let mut mapping = MappingRetention {
            path: "realm/device_id/com.Foo/bar".into(),
            version_major: 1,
            qos: Reliability::Guaranteed,
            expiry: Some(Duration::from_secs(u64::MAX)),
        };
        store.store_mapping(&mapping).await.unwrap();

        let res = store.mapping(&mapping.path).await.unwrap().unwrap();

        mapping.expiry = None;

        assert_eq!(res, mapping);
    }

    #[test]
    fn should_get_id_from_row() {
        let exp_t = 42u128;
        let exp_c = 9;
        let t = exp_t.to_be_bytes();

        let id = Id::from_row(t.as_slice(), exp_c).unwrap();

        assert_eq!(id.timestamp, exp_t);
        assert_eq!(id.counter, exp_c);
    }

    #[tokio::test]
    async fn should_store_and_check_packet() {
        let dir = tempfile::tempdir().unwrap();

        let store = SqliteStore::connect(dir.path()).await.unwrap();

        let topic = "realm/device_id/com.Foo/bar";

        let mapping = MappingRetention {
            path: topic.into(),
            version_major: 1,
            qos: Reliability::Guaranteed,
            expiry: None,
        };
        store.store_mapping(&mapping).await.unwrap();

        let packet = MappingPacket {
            id: Id {
                timestamp: 1,
                counter: 2,
            },
            path: topic.into(),
            payload: [].as_slice().into(),
        };
        store.store_packet(&packet).await.unwrap();

        let res = store.packet(&packet.id).await.unwrap().unwrap();

        assert_eq!(res, packet);
    }

    #[tokio::test]
    async fn should_remove_sent_packet() {
        let dir = tempfile::tempdir().unwrap();

        let store = SqliteStore::connect(dir.path()).await.unwrap();

        let topic = "realm/device_id/com.Foo/bar";

        let mapping = MappingRetention {
            path: topic.into(),
            version_major: 1,
            qos: Reliability::Guaranteed,
            expiry: None,
        };
        store.store_mapping(&mapping).await.unwrap();

        let id = Id {
            timestamp: 1,
            counter: 1,
        };
        let exp = MappingPacket {
            id,
            path: topic.into(),
            payload: [].as_slice().into(),
        };
        store.store_packet(&exp).await.unwrap();

        store.packet_received(&id).await.unwrap();

        let packet = store.packet(&exp.id).await.unwrap();

        assert_eq!(packet, None);
    }

    #[tokio::test]
    async fn error_remove_missing_sent_packet() {
        let dir = tempfile::tempdir().unwrap();

        let store = SqliteStore::connect(dir.path()).await.unwrap();

        let id = Id {
            timestamp: 1,
            counter: 1,
        };
        store.packet_received(&id).await.unwrap_err();
    }

    #[tokio::test]
    async fn should_get_all_packets() {
        let dir = tempfile::tempdir().unwrap();

        let store = SqliteStore::connect(dir.path()).await.unwrap();

        let topic = "realm/device_id/com.Foo/bar";

        let mapping = MappingRetention {
            path: topic.into(),
            version_major: 1,
            qos: Reliability::Guaranteed,
            expiry: None,
        };
        store.store_mapping(&mapping).await.unwrap();

        let packets = [
            MappingPacket {
                id: Id {
                    timestamp: 1,
                    counter: 2,
                },
                path: topic.into(),
                payload: [].as_slice().into(),
            },
            MappingPacket {
                id: Id {
                    timestamp: 2,
                    counter: 0,
                },
                path: topic.into(),
                payload: [].as_slice().into(),
            },
            MappingPacket {
                id: Id {
                    timestamp: 2,
                    counter: 1,
                },
                path: topic.into(),
                payload: [].as_slice().into(),
            },
        ];

        for packet in &packets {
            store.store_packet(packet).await.unwrap();
        }

        let expected = packets
            .into_iter()
            .map(|p| StoredPacket {
                qos: mapping.qos,
                id: p.id,
                topic: p.path.to_string(),
                payload: p.payload.to_vec(),
            })
            .collect_vec();

        let mut t = store.db_conn.begin().await.unwrap();
        let res = store.all_packets(&mut t).await.unwrap();
        t.commit().await.unwrap();

        assert_eq!(res, expected);
    }

    #[tokio::test]
    async fn should_resend_all() {
        let dir = tempfile::tempdir().unwrap();

        let store = SqliteStore::connect(dir.path()).await.unwrap();

        let topic = "realm/device_id/com.Foo/bar";

        let mapping = MappingRetention {
            path: topic.into(),
            version_major: 1,
            qos: Reliability::Guaranteed,
            expiry: None,
        };
        store.store_mapping(&mapping).await.unwrap();

        let packets = [
            MappingPacket {
                id: Id {
                    timestamp: 1,
                    counter: 2,
                },
                path: topic.into(),
                payload: [].as_slice().into(),
            },
            MappingPacket {
                id: Id {
                    timestamp: 2,
                    counter: 0,
                },
                path: topic.into(),
                payload: [].as_slice().into(),
            },
            MappingPacket {
                id: Id {
                    timestamp: 2,
                    counter: 1,
                },
                path: topic.into(),
                payload: [].as_slice().into(),
            },
        ];

        for packet in &packets {
            store.store_packet(packet).await.unwrap();
        }

        let expected = packets
            .into_iter()
            .map(|p| StoredPacket {
                qos: mapping.qos,
                id: p.id,
                topic: p.path.to_string(),
                payload: p.payload.to_vec(),
            })
            .collect_vec();

        let res = store.resend_all().await.unwrap();

        assert_eq!(res, expected);
    }
}
