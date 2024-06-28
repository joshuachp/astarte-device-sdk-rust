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

//! Stored interface retention.
//!
//! When available it will use the SQLite database to store the interface retention to disk, so that
//! the data is guarantied to be delivered in the time-frame specified by the expiry even after
//! shutdowns or reboots.
//!
//! When an interface major version is updated the retention cache must be invalidated. Since the
//! payload will be publish on the new introspection.

pub(crate) mod specialization;

use std::{
    array::TryFromSliceError,
    borrow::Cow,
    fmt::Display,
    time::{Duration, SystemTime, SystemTimeError, UNIX_EPOCH},
};

use async_trait::async_trait;
use tracing::error;

use crate::{error::Report, interface::Reliability};

/// Error returned by the retention.
#[derive(Debug, thiserror::Error)]
pub enum RetentionError {
    /// Invalid packet id
    #[error(transparent)]
    PacketId(#[from] PacketIdError),
    /// QoS must not be at most once (0)
    #[error("invalid QoS ({0})")]
    Qos(u8),
    /// Couldn't store the mapping
    #[error("couldn't store mapping {topic}")]
    StoreRetention {
        /// The source of the error
        #[source]
        backtrace: sqlx::Error,
        /// Topic of the mapping
        topic: String,
    },
    /// Couldn't store the payload
    #[error("couldn't store payload {topic}")]
    StorePacket {
        /// The source of the error
        #[source]
        backtrace: sqlx::Error,
        /// Topic of the packet
        topic: String,
    },
    /// Couldn't fetch mapping
    #[error("couldn't fetch mapping {topic}")]
    Mapping {
        /// The source of the error
        #[source]
        backtrace: sqlx::Error,
        /// Topic of the packet
        topic: String,
    },
    /// Couldn't fetch packet
    #[error("couldn't fetch packet {id}")]
    Packet {
        /// The source of the error
        #[source]
        backtrace: sqlx::Error,
        /// Id of the packet
        id: Id,
    },
    /// Couldn't convert timestamp bytes
    #[error("couldn't convert timestamp bytes")]
    Timestamp(#[source] TryFromSliceError),
    /// Couldn't get the next packet
    #[error("couldn't get the next packet")]
    NextPacket(#[source] sqlx::Error),
    /// Couldn't acquire transaction
    #[error("couldn't acquire transaction")]
    Transaction(#[source] sqlx::Error),
    /// Couldn't mark packet as received.
    #[error("coudln't mark packet with id {id} as received")]
    PacketReceived {
        /// The source of the error
        #[source]
        backtrace: sqlx::Error,
        /// Id of the packet
        id: Id,
    },
    /// Couldn't delete a received packet.
    #[error("error while deleting received packet {id}, rows affected {rows}")]
    DeleteReceived {
        /// Id of the packet
        id: Id,
        /// Rows modified
        rows: u64,
    },
    /// Couldn't invalidate previous session packets
    #[error("couldn't invalidate previous session packets")]
    Invalidate(#[source] sqlx::Error),
    /// Couldn't commit the resend transaction
    #[error("couldn't commit the resend transaction")]
    ResendAll(#[source] sqlx::Error),
    /// Couldn't get all the packets.
    #[error("couldn't get all the packets")]
    AllPackets(#[source] sqlx::Error),
}

/// Invalid packet id.
#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("invalid packet id {value}")]
pub struct PacketIdError {
    /// The id.
    pub value: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct MappingRetention<'a> {
    topic: Cow<'a, str>,
    version_major: i32,
    qos: Reliability,
    expiry: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct MappingPacket<'a> {
    id: Id,
    topic: Cow<'a, str>,
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

/// Trait to store application packet for a connection.
#[async_trait]
pub(crate) trait StoredRetention {
    /// Store a mapping.
    ///
    /// This will overwrite the existing one.
    async fn store_mapping(&self, mapping: &MappingRetention<'_>) -> Result<(), RetentionError>;

    /// Store a packet.
    ///
    /// This will fail if there is an existing.
    async fn store_packet(&self, packet: &MappingPacket<'_>) -> Result<(), RetentionError>;

    /// Get the mapping if it exists.
    async fn mapping(
        &self,
        topic: &str,
    ) -> Result<Option<MappingRetention<'static>>, RetentionError>;

    /// Get the mapping if it exists.
    async fn packet(&self, id: &Id) -> Result<Option<MappingPacket<'static>>, RetentionError>;

    /// Removes the packet with the given [`PacketId`] from the retention.
    async fn packet_received(&self, id: &Id) -> Result<(), RetentionError>;

    /// Resend all the packets for a new connection.
    async fn resend_all(&self) -> Result<Vec<StoredPacket>, RetentionError>;
}

#[cfg(test)]
mod tests {
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
}
