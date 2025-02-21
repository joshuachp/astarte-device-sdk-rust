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

//! Mappings for Datastream with object aggregation
//!
//! Data sent on an object interface is grouped and sent together in a single message.

use crate::{
    mapping::{endpoint::Endpoint, InterfaceMapping, MappingError},
    schema::{Mapping, MappingType},
};

/// Shared struct for a mapping for all interface types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatastreamObjectMapping {
    pub(crate) endpoint: Endpoint<String>,
    pub(crate) mapping_type: MappingType,
    #[cfg(feature = "doc-fields")]
    pub(crate) description: Option<String>,
    #[cfg(feature = "doc-fields")]
    pub(crate) doc: Option<String>,
}

impl InterfaceMapping for DatastreamObjectMapping {
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

impl<T> TryFrom<Mapping<T>> for DatastreamObjectMapping
where
    T: AsRef<str> + Into<String>,
{
    type Error = MappingError;

    fn try_from(value: Mapping<T>) -> Result<Self, Self::Error> {
        let endpoint = Endpoint::try_from(value.endpoint.as_ref())?;

        // Check that the mapping has at least two components
        // https://docs.astarte-platform.org/astarte/latest/030-interface.html#endpoints-and-aggregation
        if endpoint.len() < 2 {
            return Err(MappingError::TooShortForObject(endpoint.to_string()));
        }

        Ok(Self {
            endpoint,
            mapping_type: value.mapping_type,
            #[cfg(feature = "doc-fields")]
            description: value.description.map(T::into),
            #[cfg(feature = "doc-fields")]
            doc: value.doc.map(T::into),
        })
    }
}
