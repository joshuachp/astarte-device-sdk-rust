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

use crate::{
    mapping::{endpoint::Endpoint, InterfaceMapping},
    schema::MappingType,
};

/// Shared struct for a mapping for all interface types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatastreamObjectMapping {
    endpoint: Endpoint<String>,
    mapping_type: MappingType,
    #[cfg(feature = "interface-doc")]
    description: Option<String>,
    #[cfg(feature = "interface-doc")]
    doc: Option<String>,
}

impl InterfaceMapping for DatastreamObjectMapping {
    fn endpoint(&self) -> &Endpoint<String> {
        &self.endpoint
    }

    fn mapping_type(&self) -> MappingType {
        self.mapping_type
    }

    #[cfg(feature = "interface-doc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "interface-doc")))]
    fn description(&self) -> Option<&str> {
        self.mapping_type
    }

    #[cfg(feature = "interface-doc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "interface-doc")))]
    fn doc(&self) -> Option<&str> {
        todo!()
    }
}

impl<T> TryFrom<&Mapping<T>> for BaseMapping
where
    T: AsRef<str> + Into<String>,
{
    type Error = Error;

    fn try_from(value: &Mapping<T>) -> Result<Self, Self::Error> {
        let endpoint = Endpoint::try_from(value.endpoint().as_ref())?;

        Ok(Self {
            endpoint,
            mapping_type: value.mapping_type(),
            #[cfg(feature = "interface-doc")]
            description: value.description().map(|t| t.as_ref().into()),
            #[cfg(feature = "interface-doc")]
            doc: value.doc().map(|t| t.as_ref().into()),
        })
    }
}
