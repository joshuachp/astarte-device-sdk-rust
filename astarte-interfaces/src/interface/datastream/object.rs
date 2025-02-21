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
    interface::{
        version::InterfaceVersion, DatabaseRetention, Introspection, MappingVec, Retention,
    },
    mapping::{
        datastream::object::DatastreamObjectMapping, endpoint::Endpoint, path::MappingPath,
        InterfaceMapping, MappingError,
    },
    schema::{Aggregation, InterfaceJson, InterfaceType, Mapping, Ownership, Reliability},
};

/// Error when parsing a [`DatastreamObject`]
#[derive(Debug, thiserror::Error)]
pub enum ObjectError {
    /// Object has a different value for the specified mapping
    #[error("object has a different {ctx} for the mapping {endpoint}")]
    Mapping { ctx: &'static str, endpoint: String },
    /// Mapping endpoint differs from others
    ///
    /// It needs to have up to the latest level equal to the others endpoints.
    ///
    /// See [the Astarte documentation](https://docs.astarte-platform.org/astarte/latest/030-interface.html#endpoints-and-aggregation)
    #[error("object has an inconsistent enpdoint {endpoint}")]
    Endpoint { endpoint: String },
}

impl ObjectError {
    fn mapping(ctx: &'static str, endpoint: impl AsRef<str>) -> Self {
        Self::Mapping {
            ctx,
            endpoint: endpoint.as_ref().to_string(),
        }
    }
}

/// Interface of type datastream object.
///
/// For this interface all the mappings have the same prefix and configurations.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DatastreamObject {
    interface_name: String,
    version: InterfaceVersion,
    ownership: Ownership,
    reliability: Reliability,
    explicit_timestamp: bool,
    retention: Retention,
    database_retention: DatabaseRetention,
    mappings: MappingVec<DatastreamObjectMapping>,
    #[cfg(feature = "interface-doc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "interface-doc")))]
    description: Option<String>,
    #[cfg(feature = "interface-doc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "interface-doc")))]
    doc: Option<String>,
}

impl Introspection for DatastreamObject {
    type Mapping = DatastreamObjectMapping;

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
        Aggregation::Object
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

impl<T> TryFrom<&InterfaceJson<T>> for DatastreamObject
where
    T: AsRef<str> + Eq,
{
    type Error = Error;

    fn try_from(value: &InterfaceJson<T>) -> Result<Self, Self::Error> {
        let version = InterfaceVersion::try_new(value.version_major, value.version_major)?;

        let first_mapping = value.mappings.first().ok_or(MappingError::Empty)?;
        let first_endpoint = Endpoint::try_from(first_mapping.endpoint.as_ref())?;

        let mappings = value
            .mappings
            .iter()
            .map(|mapping| {
                are_mapping_compatible(first_mapping, mapping)?;

                let mapping = DatastreamObjectMapping::try_from(mapping)?;

                if !mapping.endpoint().is_same_object(&first_endpoint) {
                    return Err(Error::Object(ObjectError::Endpoint {
                        endpoint: mapping.endpoint().to_string(),
                    }));
                }

                Ok(mapping)
            })
            .collect::<Result<Vec<_>, Error>>()?;

        let mappings = MappingVec::try_from(mappings)?;

        // We create the object from the first mapping and then insert the others, checking if
        // compatible
        let object = Self {
            reliability: first_mapping.reliability.unwrap_or_default(),
            explicit_timestamp: first_mapping.explicit_timestamp.unwrap_or_default(),
            retention: first_mapping.retention_with_expiry(),
            database_retention: first_mapping.database_retention_with_ttl(),
            mappings,
            interface_name: value.interface_name.as_ref().to_string(),
            version,
            ownership: value.ownership,
            #[cfg(feature = "interface-doc")]
            description: value.description.as_ref().map(|v| v.as_ref().to_string()),
            #[cfg(feature = "interface-doc")]
            doc: value.doc.as_ref().map(|v| v.as_ref().to_string()),
        };

        Ok(object)
    }
}

fn are_mapping_compatible<T>(a: &Mapping<T>, b: &Mapping<T>) -> Result<(), ObjectError>
where
    T: AsRef<str> + Eq,
{
    if a.reliability != b.reliability {
        return Err(ObjectError::mapping("reliability", &b.endpoint));
    }

    if a.explicit_timestamp != b.explicit_timestamp {
        return Err(ObjectError::mapping("explicit_timestamp", &b.endpoint));
    }

    if a.retention != b.retention {
        return Err(ObjectError::mapping("retention", &b.endpoint));
    }

    if a.expiry != b.expiry {
        return Err(ObjectError::mapping("expiry", &b.endpoint));
    }

    if a.database_retention_policy != b.database_retention_policy {
        return Err(ObjectError::mapping(
            "database_retention_policy",
            &b.endpoint,
        ));
    }

    if a.database_retention_ttl != b.database_retention_ttl {
        return Err(ObjectError::mapping("database_retention_ttl", &b.endpoint));
    }

    Ok(())
}
