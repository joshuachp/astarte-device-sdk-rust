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

use tracing::warn;

use crate::{
    interface::{DatabaseRetention, Retention},
    mapping::{endpoint::Endpoint, InterfaceMapping, MappingError},
    schema::{Mapping, MappingType, Reliability},
};

/// Mapping of a [`DatastreamIndividual`](super::DatastreamIndividual) interface.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DatastreamIndividualMapping {
    endpoint: Endpoint<String>,
    mapping_type: MappingType,
    reliability: Reliability,
    retention: Retention,
    database_retention: DatabaseRetention,
    explicit_timestamp: bool,
    #[cfg(feature = "interface-doc")]
    description: Option<String>,
    #[cfg(feature = "interface-doc")]
    doc: Option<String>,
}

impl InterfaceMapping for DatastreamIndividualMapping {
    fn endpoint(&self) -> &Endpoint<String> {
        &self.endpoint
    }

    fn mapping_type(&self) -> MappingType {
        self.mapping_type
    }

    #[cfg(feature = "interface-doc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "interface-doc")))]
    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    #[cfg(feature = "interface-doc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "interface-doc")))]
    fn doc(&self) -> Option<&str> {
        self.doc.as_deref()
    }
}

impl<T> TryFrom<&Mapping<T>> for DatastreamIndividualMapping
where
    T: AsRef<str> + Into<String>,
{
    type Error = MappingError;

    fn try_from(value: &Mapping<T>) -> Result<Self, Self::Error> {
        let endpoint = Endpoint::try_from(value.endpoint.as_ref())?;

        if value.allow_unset {
            warn!("datastream cannot have allow_unset, ignoring");
        }

        Ok(Self {
            endpoint,
            reliability: value.reliability,
            retention: value.retention_with_expiry(),
            database_retention: value.database_retention_with_ttl(),
            explicit_timestamp: value.explicit_timestamp,
            mapping_type: value.mapping_type,
            #[cfg(feature = "interface-doc")]
            description: value.description.map(T::into),
            #[cfg(feature = "interface-doc")]
            doc: value.doc.map(T::into),
        })
    }
}
