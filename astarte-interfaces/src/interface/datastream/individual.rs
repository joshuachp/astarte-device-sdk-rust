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
    error::Error,
    interface::{version::InterfaceVersion, Introspection, MappingVec},
    mapping::{
        datastream::individual::DatastreamIndividualMapping, path::MappingPath, MappingError,
    },
    schema::{Aggregation, InterfaceJson, InterfaceType, Ownership},
};

/// Interface of type datastream individual.
///
/// For this interface all the mappings have distinct configurations.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DatastreamIndividual {
    interface_name: String,
    version: InterfaceVersion,
    ownership: Ownership,
    mappings: MappingVec<DatastreamIndividualMapping>,
    #[cfg(feature = "interface-doc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "interface-doc")))]
    description: Option<String>,
    #[cfg(feature = "interface-doc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "interface-doc")))]
    doc: Option<String>,
}

impl Introspection for DatastreamIndividual {
    type Mapping = DatastreamIndividualMapping;

    fn interface_name(&self) -> &str {
        &self.interface_name
    }

    fn version_major(&self) -> i32 {
        self.version.version_major()
    }

    fn version_minor(&self) -> i32 {
        self.version.version_minor()
    }

    fn version(&self) -> InterfaceVersion {
        self.version
    }

    fn interface_type(&self) -> InterfaceType {
        InterfaceType::Datastream
    }

    fn ownership(&self) -> Ownership {
        self.ownership
    }

    fn aggregation(&self) -> Aggregation {
        Aggregation::Individual
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

    fn iter_mappings(&self) -> impl Iterator<Item = &Self::Mapping> {
        self.mappings.iter()
    }

    fn mapping(&self, path: &MappingPath) -> Option<&Self::Mapping> {
        self.mappings.get(path)
    }

    fn mappings_len(&self) -> usize {
        self.mappings.len()
    }
}

impl<T> TryFrom<&InterfaceJson<T>> for DatastreamIndividual
where
    T: AsRef<str>,
{
    type Error = Error;

    fn try_from(value: &InterfaceJson<T>) -> Result<Self, Self::Error> {
        let version = InterfaceVersion::try_new(value.version_major, value.version_minor)?;

        let mappings = value
            .mappings
            .iter()
            .map(DatastreamIndividualMapping::try_from)
            .collect::<Result<Vec<_>, MappingError>>()
            .and_then(MappingVec::try_from)?;

        Ok(Self {
            interface_name: value.interface_name.as_ref().to_string(),
            version,
            ownership: value.ownership,
            mappings,
            #[cfg(feature = "interface-doc")]
            description: value.description.as_ref().map(|v| v.as_ref().to_string()),
            #[cfg(feature = "interface-doc")]
            doc: value.doc.as_ref().map(|v| v.as_ref().to_string()),
        })
    }
}
