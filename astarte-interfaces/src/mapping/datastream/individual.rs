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

//! Mapping for Datastream with aggregation individual.
//!
//! In case aggregation is individual, each mapping is treated as an independent value and is
//! managed individually.

use std::borrow::Cow;

use tracing::warn;

use crate::{
    error::Error,
    interface::{DatabaseRetention, Retention},
    mapping::{endpoint::Endpoint, InterfaceMapping},
    schema::{Mapping, MappingType, Reliability},
};

/// Mapping of a [`DatastreamIndividual`](super::DatastreamIndividual) interface.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DatastreamIndividualMapping {
    pub(crate) endpoint: Endpoint<String>,
    pub(crate) mapping_type: MappingType,
    pub(crate) reliability: Reliability,
    pub(crate) retention: Retention,
    pub(crate) database_retention: DatabaseRetention,
    pub(crate) explicit_timestamp: bool,
    #[cfg(feature = "doc-fields")]
    pub(crate) description: Option<String>,
    #[cfg(feature = "doc-fields")]
    pub(crate) doc: Option<String>,
}

impl DatastreamIndividualMapping {
    /// Returns the [`Reliability`] of the mapping.
    #[must_use]
    pub fn reliability(&self) -> Reliability {
        self.reliability
    }

    /// Returns the [`Retention`] of the mapping.
    #[must_use]
    pub fn retention(&self) -> Retention {
        self.retention
    }

    /// Returns the [`DatabaseRetention`] of the mapping.
    #[must_use]
    pub fn database_retention(&self) -> DatabaseRetention {
        self.database_retention
    }

    /// Returns true if the mapping requires an explicit timestamp.
    ///
    /// Otherwise the reception timestamp is used.
    #[must_use]
    pub fn explicit_timestamp(&self) -> bool {
        self.explicit_timestamp
    }
}

impl InterfaceMapping for DatastreamIndividualMapping {
    fn endpoint(&self) -> &Endpoint<String> {
        &self.endpoint
    }

    fn mapping_type(&self) -> MappingType {
        self.mapping_type
    }

    #[cfg(feature = "doc-fields")]
    #[cfg_attr(docsrs, doc(cfg(feature = "doc-fields")))]
    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    #[cfg(feature = "doc-fields")]
    #[cfg_attr(docsrs, doc(cfg(feature = "doc-fields")))]
    fn doc(&self) -> Option<&str> {
        self.doc.as_deref()
    }
}

impl<T> TryFrom<Mapping<T>> for DatastreamIndividualMapping
where
    T: AsRef<str> + Into<String>,
{
    type Error = Error;

    fn try_from(value: Mapping<T>) -> Result<Self, Self::Error> {
        let endpoint = Endpoint::try_from(value.endpoint.as_ref())?;
        let retention = value.retention_with_expiry()?;
        let database_retention = value.database_retention_with_ttl()?;

        // TODO: strict
        if value.allow_unset.is_some() {
            warn!("datastream cannot have allow_unset, ignoring");
        }

        Ok(Self {
            endpoint,
            reliability: value.reliability.unwrap_or_default(),
            retention,
            database_retention,
            explicit_timestamp: value.explicit_timestamp.unwrap_or_default(),
            mapping_type: value.mapping_type,
            #[cfg(feature = "doc-fields")]
            description: value.description.map(T::into),
            #[cfg(feature = "doc-fields")]
            doc: value.doc.map(T::into),
        })
    }
}

impl<'a> From<&'a DatastreamIndividualMapping> for Mapping<Cow<'a, str>> {
    fn from(value: &'a DatastreamIndividualMapping) -> Self {
        Mapping {
            endpoint: value.endpoint.to_string().into(),
            mapping_type: value.mapping_type,
            reliability: value.reliability.into(),
            explicit_timestamp: Some(value.explicit_timestamp),
            retention: Some(value.retention.into()),
            expiry: value.retention.as_expiry_seconds(),
            database_retention_policy: Some(value.database_retention.into()),
            database_retention_ttl: value.database_retention.as_ttl_secs(),
            allow_unset: None,
            #[cfg(feature = "doc-fields")]
            description: value.description().map(Cow::Borrowed),
            #[cfg(feature = "doc-fields")]
            doc: value.doc().map(Cow::Borrowed),
            #[cfg(not(feature = "doc-fields"))]
            description: None,
            #[cfg(not(feature = "doc-fields"))]
            doc: None,
        }
    }
}
