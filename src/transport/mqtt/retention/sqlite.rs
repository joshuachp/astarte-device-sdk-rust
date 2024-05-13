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

use std::time::Duration;

use async_trait::async_trait;
use futures::TryStreamExt;
use sqlx::{query_file, Sqlite, Transaction};

use crate::{interface::Reliability, store::SqliteStore};

use super::{Id, MappingPacket, MappingRetention, RetentionError, StoredPacket, StoredRetention};

/// Gets the [`Reliability`] from a stored [`u8`].
fn reliability_from_row(qos: u8) -> Result<Reliability, RetentionError> {
    match qos {
        1 => Ok(Reliability::Guaranteed),
        2 => Ok(Reliability::Unique),
        qos => Err(RetentionError::Qos(qos)),
    }
}

// Implementation of utilities used in the retention
impl SqliteStore {
    /// Check if a [`PacketId`] already exists.
    async fn all_packets(
        &self,
        transaction: &mut Transaction<'_, Sqlite>,
    ) -> Result<Vec<StoredPacket>, RetentionError> {
        query_file!("queries/retention/all_packets.sql")
            .fetch(&mut **transaction)
            .map_err(RetentionError::AllPackets)
            .and_then(|row| async move {
                let id =
                    Id::from_row(&row.t_millis, row.counter).map_err(RetentionError::Timestamp)?;

                let qos = reliability_from_row(row.qos)?;

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
}

#[async_trait]
impl StoredRetention for SqliteStore {
    async fn store_mapping(&self, mapping: &MappingRetention<'_>) -> Result<(), RetentionError> {
        let qos: u8 = match mapping.qos {
            Reliability::Unreliable => return Err(RetentionError::Qos(0)),
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
            mapping.topic,
            mapping.version_major,
            qos,
            exp
        )
        .execute(&self.db_conn)
        .await
        .map_err(|err| RetentionError::StoreRetention {
            backtrace: err,
            topic: mapping.topic.to_string(),
        })?;

        Ok(())
    }

    async fn store_packet(&self, packet: &MappingPacket<'_>) -> Result<(), RetentionError> {
        let be_bytes = packet.id.timestamp.to_be_bytes();
        let timestamp = be_bytes.as_slice();
        let counter = packet.id.counter;

        let payload: &[u8] = &packet.payload;

        query_file!(
            "queries/retention/store_packet.sql",
            timestamp,
            counter,
            packet.topic,
            payload,
        )
        .execute(&self.db_conn)
        .await
        .map_err(|err| RetentionError::StorePacket {
            backtrace: err,
            topic: packet.topic.to_string(),
        })?;

        Ok(())
    }

    async fn mapping(
        &self,
        topic: &str,
    ) -> Result<Option<MappingRetention<'static>>, RetentionError> {
        let Some(row) = query_file!("queries/retention/mapping.sql", topic)
            .fetch_optional(&self.db_conn)
            .await
            .map_err(|err| RetentionError::Mapping {
                backtrace: err,
                topic: topic.to_string(),
            })?
        else {
            return Ok(None);
        };

        let qos = reliability_from_row(row.qos)?;

        let expiry = row.expiry_sec.and_then(|exp| {
            // If the conversion fails, let's keep the packet forever.
            exp.try_into().ok().map(Duration::from_secs)
        });

        Ok(Some(MappingRetention {
            topic: row.topic.into(),
            version_major: row.major_version,
            qos,
            expiry,
        }))
    }

    async fn packet(&self, id: &Id) -> Result<Option<MappingPacket<'static>>, RetentionError> {
        let timestamp = id.timestamp_as_bytes();
        let timestamp = timestamp.as_slice();

        let Some(row) = query_file!("queries/retention/packet.sql", timestamp, id.counter)
            .fetch_optional(&self.db_conn)
            .await
            .map_err(|err| RetentionError::Packet {
                backtrace: err,
                id: *id,
            })?
        else {
            return Ok(None);
        };

        let id = Id::from_row(&row.t_millis, row.counter).map_err(RetentionError::Timestamp)?;

        Ok(Some(MappingPacket {
            id,
            topic: row.topic.into(),
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

    #[tokio::test]
    async fn should_store_and_check_mapping() {
        let dir = tempfile::tempdir().unwrap();

        let store = SqliteStore::connect(dir.path()).await.unwrap();

        let mapping = MappingRetention {
            topic: "realm/device_id/com.Foo/bar".into(),
            version_major: 1,
            qos: Reliability::Guaranteed,
            expiry: None,
        };
        store.store_mapping(&mapping).await.unwrap();

        let res = store.mapping(&mapping.topic).await.unwrap().unwrap();

        assert_eq!(res, mapping);
    }

    #[tokio::test]
    async fn should_replace_mapping() {
        let dir = tempfile::tempdir().unwrap();

        let store = SqliteStore::connect(dir.path()).await.unwrap();

        let mut mapping = MappingRetention {
            topic: "realm/device_id/com.Foo/bar".into(),
            version_major: 1,
            qos: Reliability::Guaranteed,
            expiry: None,
        };
        store.store_mapping(&mapping).await.unwrap();

        let res = store.mapping(&mapping.topic).await.unwrap().unwrap();

        assert_eq!(res, mapping);

        mapping.version_major = 2;

        store.store_mapping(&mapping).await.unwrap();

        let res = store.mapping(&mapping.topic).await.unwrap().unwrap();

        assert_eq!(res, mapping);
    }

    #[tokio::test]
    async fn expiry_too_big() {
        let dir = tempfile::tempdir().unwrap();

        let store = SqliteStore::connect(dir.path()).await.unwrap();

        let mut mapping = MappingRetention {
            topic: "realm/device_id/com.Foo/bar".into(),
            version_major: 1,
            qos: Reliability::Guaranteed,
            expiry: Some(Duration::from_secs(u64::MAX)),
        };
        store.store_mapping(&mapping).await.unwrap();

        let res = store.mapping(&mapping.topic).await.unwrap().unwrap();

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
            topic: topic.into(),
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
            topic: topic.into(),
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
            topic: topic.into(),
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
            topic: topic.into(),
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
            topic: topic.into(),
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
                topic: topic.into(),
                payload: [].as_slice().into(),
            },
            MappingPacket {
                id: Id {
                    timestamp: 2,
                    counter: 0,
                },
                topic: topic.into(),
                payload: [].as_slice().into(),
            },
            MappingPacket {
                id: Id {
                    timestamp: 2,
                    counter: 1,
                },
                topic: topic.into(),
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
                topic: p.topic.to_string(),
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
            topic: topic.into(),
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
                topic: topic.into(),
                payload: [].as_slice().into(),
            },
            MappingPacket {
                id: Id {
                    timestamp: 2,
                    counter: 0,
                },
                topic: topic.into(),
                payload: [].as_slice().into(),
            },
            MappingPacket {
                id: Id {
                    timestamp: 2,
                    counter: 1,
                },
                topic: topic.into(),
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
                topic: p.topic.to_string(),
                payload: p.payload.to_vec(),
            })
            .collect_vec();

        let res = store.resend_all().await.unwrap();

        assert_eq!(res, expected);
    }
}
