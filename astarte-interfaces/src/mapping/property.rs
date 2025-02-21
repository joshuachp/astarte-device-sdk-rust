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

use crate::schema::{Mapping, MappingType};

use super::{endpoint::Endpoint, InterfaceMapping, MappingError};

/// Mapping of a [`Properties`](super::Properties) interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyMapping {
    endpoint: Endpoint<String>,
    mapping_type: MappingType,
    allow_unset: bool,
    #[cfg(feature = "interface-doc")]
    description: Option<String>,
    #[cfg(feature = "interface-doc")]
    doc: Option<String>,
}

impl InterfaceMapping for PropertyMapping {
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

impl<T> TryFrom<&Mapping<T>> for PropertyMapping
where
    T: AsRef<str>,
{
    type Error = MappingError;

    fn try_from(value: &Mapping<T>) -> Result<Self, Self::Error> {
        let endpoint = Endpoint::try_from(value.endpoint.as_ref())?;

        if value.reliability.is_some() {
            warn!("property cannot have explicit_timestamp, ignoring");
        }

        if value.explicit_timestamp.is_some() {
            warn!("property cannot have explicit_timestamp, ignoring");
        }

        if value.explicit_timestamp.is_some() {
            warn!("property cannot have explicit_timestamp, ignoring");
        }

        Ok(Self {
            endpoint,
            mapping_type: value.mapping_type,
            allow_unset: value.allow_unset,
            #[cfg(feature = "interface-doc")]
            description: value.description.as_ref().map(|t| t.as_ref().to_string()),
            #[cfg(feature = "interface-doc")]
            doc: value.doc.as_ref().map(|t| t.as_ref().to_string()),
        })
    }
}
