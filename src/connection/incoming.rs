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

use tracing::{debug, warn};

use crate::client::RecvError;
use crate::error::AggregationError;
use crate::interface::mapping::path::MappingPath;
use crate::interface::Aggregation;
use crate::store::{PropertyMapping, PropertyStore, StoredProp};
use crate::transport::{Connection, Receive, ReceivedEvent, TransportError};
use crate::{DeviceEvent, Error, Interface, Value};

use super::DeviceConnection;

impl<C, S> DeviceConnection<C, S>
where
    C: Connection,
{
    async fn handle_event(
        &self,
        interface: &str,
        path: &str,
        payload: C::Payload,
    ) -> Result<Value, TransportError>
    where
        S: PropertyStore,
        C: Receive + Sync,
    {
        let path = MappingPath::try_from(path)
            .map_err(|err| TransportError::Recv(RecvError::InvalidEndpoint(err)))?;

        let interfaces = self.state.interfaces.read().await;
        let Some(interface) = interfaces.get(interface) else {
            warn!("publish on missing interface {interface} ({path})");
            return Err(TransportError::Recv(RecvError::InterfaceNotFound {
                name: interface.to_string(),
            }));
        };

        let (data, timestamp) = match interface.aggregation() {
            Aggregation::Individual => {
                self.handle_payload_individual(interface, &path, payload)
                    .await?
            }
            Aggregation::Object => {
                self.handle_payload_object(interface, &path, payload)
                    .await?
            }
        };

        debug!("received {{v: {data:?}, t: {timestamp:?}}}");

        Ok(data)
    }

    /// Handles the payload of an interface with [`InterfaceAggregation::Individual`]
    async fn handle_payload_individual(
        &self,
        interface: &Interface,
        path: &MappingPath<'_>,
        payload: C::Payload,
    ) -> Result<(Value, Option<chrono::DateTime<chrono::Utc>>), TransportError>
    where
        S: PropertyStore,
        C: Receive + Sync,
    {
        let Some(mapping) = interface.as_mapping_ref(path) else {
            return Err(TransportError::Recv(RecvError::MappingNotFound {
                interface: interface.interface_name().to_string(),
                mapping: path.to_string(),
            }));
        };

        let individual = self.connection.deserialize_individual(&mapping, payload)?;

        match individual {
            Some((value, timestamp)) => {
                if let Some(prop) = mapping.as_prop() {
                    let prop = StoredProp::from_mapping(&prop, &value);

                    self.store
                        .store_prop(prop)
                        .await
                        .map_err(|err| TransportError::Transport(Error::Store(err)))?;

                    debug!(
                        "property stored {}{path}:{}",
                        interface.interface_name(),
                        interface.version_major()
                    );
                }

                Ok((Value::Individual(value), timestamp))
            }
            None => {
                // Unset can only be received for a property
                self.store
                    .delete_prop(&PropertyMapping::new_unchecked(
                        interface.into(),
                        path.as_str(),
                    ))
                    .await
                    .map_err(|err| TransportError::Transport(Error::Store(err)))?;

                debug!(
                    "property unset {}{path}:{}",
                    interface.interface_name(),
                    interface.version_major()
                );

                Ok((Value::Unset, None))
            }
        }
    }

    /// Handles the payload of an interface with [`InterfaceAggregation::Object`]
    async fn handle_payload_object(
        &self,
        interface: &Interface,
        path: &MappingPath<'_>,
        payload: C::Payload,
    ) -> Result<(Value, Option<chrono::DateTime<chrono::Utc>>), TransportError>
    where
        S: PropertyStore,
        C: Receive + Sync,
    {
        let Some(object) = interface.as_object_ref() else {
            let aggr_err = AggregationError::new(
                interface.interface_name(),
                path.as_str(),
                Aggregation::Object,
                Aggregation::Individual,
            );
            return Err(TransportError::Recv(RecvError::Aggregation(aggr_err)));
        };

        let (data, timestamp) = self.connection.deserialize_object(&object, path, payload)?;

        Ok((Value::Object(data), timestamp))
    }

    pub(super) async fn handle_connection_event(
        &self,
        event: ReceivedEvent<C::Payload>,
    ) -> Result<(), Error>
    where
        C: Receive + Sync,
        S: PropertyStore,
    {
        let data = match self
            .handle_event(&event.interface, &event.path, event.payload)
            .await
        {
            Ok(aggregation) => Ok(DeviceEvent {
                interface: event.interface,
                path: event.path,
                data: aggregation,
            }),
            Err(TransportError::Recv(recv_err)) => Err(recv_err),
            Err(TransportError::Transport(err)) => {
                return Err(err);
            }
        };

        self.tx
            .send_async(data)
            .await
            .map_err(|_| Error::Disconnected)
    }
}
