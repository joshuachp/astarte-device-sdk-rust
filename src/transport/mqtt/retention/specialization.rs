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

//! Specialize the store retention for the underling store.

use async_trait::async_trait;

use crate::store::{wrapper::StoreWrapper, PropertyStore, SqliteStore};

use super::{Id, MappingPacket, MappingRetention, RetentionError, StoredPacket, StoredRetention};

/// Work around the missing specialization feature in rust
///
/// We will use `autoderf` to access the correct specialized method.
#[async_trait]
pub(crate) trait NoRetention {
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

#[async_trait]
impl<S> NoRetention for StoreWrapper<S>
where
    S: PropertyStore,
{
    async fn store_mapping(&self, _mapping: &MappingRetention<'_>) -> Result<(), RetentionError> {
        Ok(())
    }

    /// Store a packet.
    ///
    /// This will fail if there is an existing
    async fn store_packet(&self, _packet: &MappingPacket<'_>) -> Result<(), RetentionError> {
        Ok(())
    }

    /// Get the mapping if it exists.
    async fn mapping(
        &self,
        _topic: &str,
    ) -> Result<Option<MappingRetention<'static>>, RetentionError> {
        Ok(None)
    }

    /// Get the mapping if it exists.
    async fn packet(&self, _id: &Id) -> Result<Option<MappingPacket<'static>>, RetentionError> {
        Ok(None)
    }

    /// Removes the packet with the given [`PacketId`] from the retention.
    async fn packet_received(&self, _id: &Id) -> Result<(), RetentionError> {
        Ok(())
    }

    /// Resend all the packets for a new connection.
    async fn resend_all(&self) -> Result<Vec<StoredPacket>, RetentionError> {
        Ok(Vec::new())
    }
}

/// Deref impl of the retention specialization
#[async_trait]
impl StoredRetention for &StoreWrapper<SqliteStore> {
    async fn store_mapping(&self, mapping: &MappingRetention<'_>) -> Result<(), RetentionError> {
        self.store.store_mapping(mapping).await
    }

    /// Store a packet.
    ///
    /// This will fail if there is an existing
    async fn store_packet(&self, packet: &MappingPacket<'_>) -> Result<(), RetentionError> {
        self.store.store_packet(packet).await
    }

    /// Get the mapping if it exists.
    async fn mapping(
        &self,
        topic: &str,
    ) -> Result<Option<MappingRetention<'static>>, RetentionError> {
        self.store.mapping(topic).await
    }

    /// Get the mapping if it exists.
    async fn packet(&self, id: &Id) -> Result<Option<MappingPacket<'static>>, RetentionError> {
        self.store.packet(id).await
    }

    /// Removes the packet with the given [`PacketId`] from the retention.
    async fn packet_received(&self, id: &Id) -> Result<(), RetentionError> {
        self.store.packet_received(id).await
    }

    /// Resend all the packets for a new connection.
    async fn resend_all(&self) -> Result<Vec<StoredPacket>, RetentionError> {
        self.store.resend_all().await
    }
}

#[cfg(test)]
mod tests {
    use crate::interface::Reliability;

    use super::*;

    #[tokio::test]
    async fn should_store_and_check_mapping() {
        let dir = tempfile::tempdir().unwrap();

        let store = &StoreWrapper::new(SqliteStore::connect(dir.path()).await.unwrap());

        let mapping = MappingRetention {
            topic: "realm/device_id/com.Foo/bar".into(),
            version_major: 1,
            qos: Reliability::Guaranteed,
            expiry: None,
        };
        (&store).store_mapping(&mapping).await.unwrap();

        let res = (&store).mapping(&mapping.topic).await.unwrap().unwrap();

        assert_eq!(res, mapping);
    }
}
