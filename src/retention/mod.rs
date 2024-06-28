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

use std::{borrow::Cow, hash::Hash, time::Duration};

use async_trait::async_trait;

use crate::interface::Reliability;

pub(crate) mod sqlite;

/// Error returned by the retention.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RetentionError {}

/// Publish information to be stored.
#[derive(Debug, Clone)]
pub struct PublishInfo<'a> {
    path: Cow<'a, str>,
    major_version: i32,
    reliability: Reliability,
    expiry: Duration,
    value: Cow<'a, [u8]>,
}

/// Trait to store application packet for a connection.
///
/// A store wants to implement this retention to implement the interfaces with retention stored for
/// the [`Mqtt`](crate::transport::mqtt::Mqtt) connection.
#[async_trait]
pub trait StoredRetention {
    type Id: Hash + Eq;

    /// Store a publish.
    async fn store_publish(&self, publish: PublishInfo) -> Result<Self::Id, RetentionError>;

    /// It will mark the stored publish as received.
    async fn mark_received(&self, packet: &Self::Id) -> Result<(), RetentionError>;

    // TODO: should we add a deletion mechanism for mismatching major versions?
    // async fn delete_old();

    /// Resend all the packets.
    async fn resend_all(&self) -> Result<Vec<PublishInfo>, RetentionError>;
}
