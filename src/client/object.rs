// This file is part of Astarte.
//
// Copyright 2025 SECO Mind Srl
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// SPDX-License-Identifier: Apache-2.0

//! Handles the sending of object datastream.

use tracing::{debug, trace, warn};

use crate::aggregate::AstarteObject;
use crate::client::ValidatedObject;
use crate::error::AggregationError;
use crate::interface::mapping::path::MappingPath;
use crate::interface::{Aggregation, Retention};
use crate::store::StoreCapabilities;
use crate::{retention, Error};

use super::{
    DeviceClient, Publish, RetentionId, SharedVolatileStore, StoreWrapper, StoredRetentionExt,
};

impl<C, S> DeviceClient<C, S> {
    pub(crate) async fn send_datastream_object(
        &mut self,
        interface_name: &str,
        path: &MappingPath<'_>,
        data: AstarteObject,
        timestamp: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<(), Error>
    where
        C: Publish,
        S: StoreCapabilities,
    {
        let interfaces = self.interfaces.read().await;
        let interface = interfaces
            .get(interface_name)
            .ok_or_else(|| Error::InterfaceNotFound {
                name: interface_name.to_string(),
            })?;

        let object = interface.as_object_ref().ok_or_else(|| {
            Error::Aggregation(AggregationError::new(
                interface_name,
                path.as_str(),
                Aggregation::Object,
                interface.aggregation(),
            ))
        })?;

        let validated = ValidatedObject::validate(object, path, data, timestamp)?;

        debug!("sending object {}{}", interface_name, path);

        if !self.status.is_connected() {
            trace!("publish object while connection is offline");

            return Self::offline_send_object(
                &self.retention_ctx,
                &self.volatile_store,
                &self.store,
                &mut self.sender,
                validated,
            )
            .await;
        }

        match validated.retention {
            Retention::Volatile { .. } => {
                Self::send_volatile_object(
                    &self.retention_ctx,
                    &self.volatile_store,
                    &mut self.sender,
                    validated,
                )
                .await
            }
            Retention::Stored { .. } => {
                Self::send_stored_object(
                    &self.retention_ctx,
                    &self.store,
                    &mut self.sender,
                    validated,
                )
                .await
            }
            Retention::Discard => self.sender.send_object(validated).await,
        }
    }

    async fn offline_send_object(
        retention_ctx: &retention::Context,
        volatile_store: &SharedVolatileStore,
        store: &StoreWrapper<S>,
        sender: &mut C,
        data: ValidatedObject,
    ) -> Result<(), Error>
    where
        C: Publish,
        S: StoreCapabilities,
    {
        match data.retention {
            Retention::Discard => {
                debug!("drop publish with retention discard since disconnected");
            }
            Retention::Volatile { .. } => {
                let id = retention_ctx.next();

                volatile_store.push(id, data).await;
            }
            Retention::Stored { .. } => {
                let id = retention_ctx.next();
                if let Some(retention) = store.get_retention() {
                    let value = sender.serialize_object(&data)?;

                    retention.store_publish_object(&id, &data, &value).await?;
                } else {
                    warn!("storing interface with retention stored in volatile since the store doesn't support retention");

                    volatile_store.push(id, data).await;
                }
            }
        }

        Ok(())
    }

    async fn send_volatile_object(
        retention_ctx: &retention::Context,
        volatile_store: &SharedVolatileStore,
        sender: &mut C,
        data: ValidatedObject,
    ) -> Result<(), Error>
    where
        C: Publish,
    {
        let id = retention_ctx.next();

        volatile_store.push(id, data.clone()).await;

        sender
            .send_object_stored(RetentionId::Volatile(id), data)
            .await
    }

    async fn send_stored_object(
        retention_ctx: &retention::Context,
        store: &StoreWrapper<S>,
        sender: &mut C,
        data: ValidatedObject,
    ) -> Result<(), Error>
    where
        C: Publish,
        S: StoreCapabilities,
    {
        let Some(retention) = store.get_retention() else {
            warn!("not storing interface with retention stored since the store doesn't support retention");

            return sender.send_object(data).await;
        };

        let value = sender.serialize_object(&data)?;

        let id = retention_ctx.next();

        retention.store_publish_object(&id, &data, &value).await?;

        sender
            .send_object_stored(RetentionId::Stored(id), data)
            .await
    }
}
