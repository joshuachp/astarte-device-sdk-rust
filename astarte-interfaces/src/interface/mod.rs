/*
 * This file is part of Astarte.
 *
 * Copyright 2021 SECO Mind Srl
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *    http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 *
 * SPDX-License-Identifier: Apache-2.0
 */

//! Provides the functionalities to parse and validate an Astarte interface.

pub mod datastream;
pub mod property;
pub mod reference;
pub mod validation;
pub mod version;

use std::fmt::Display;
use std::str::FromStr;
use std::time::Duration;

use datastream::object::DatastreamObject;
use property::Property;
use serde::{Deserialize, Serialize};
use tracing::info;

use self::datastream::individual::DatastreamIndividual;
use self::validation::VersionChange;
use self::version::InterfaceVersion;
use crate::error::Error;
use crate::mapping::{collection::MappingVec, path::MappingPath};
use crate::schema::InterfaceJson;
use crate::schema::{Aggregation, InterfaceType, Ownership};

/// Maximum number of mappings an interface can have
///
/// See the [Astarte interface scheme](https://docs.astarte-platform.org/latest/040-interface_schema.html#astarte-interface-schema-mappings)
pub const MAX_INTERFACE_MAPPINGS: usize = 1024;

pub trait Introspection {
    type Mapping: Sized;

    /// Returns the interface name.
    fn interface_name(&self) -> &str;
    /// Returns the interface major version.
    fn version_major(&self) -> i32;
    /// Returns the interface minor version.
    fn version_minor(&self) -> i32;
    /// Returns the interface version.
    fn version(&self) -> InterfaceVersion;
    /// Returns the interface type.
    fn interface_type(&self) -> InterfaceType;
    /// Returns the interface ownership.
    fn ownership(&self) -> Ownership;
    /// Returns the interface aggregation.
    fn aggregation(&self) -> Aggregation;

    #[cfg(feature = "interface-doc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "interface-doc")))]
    /// Returns the interface description
    fn description(&self) -> Option<&str>;

    #[cfg(feature = "interface-doc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "interface-doc")))]
    /// Returns the interface documentation.
    fn doc(&self) -> Option<&str>;

    /// Returns an iterator over the interface's mappings.
    fn iter_mappings(&self) -> impl Iterator<Item = &Self::Mapping>;

    fn mapping(&self, path: &MappingPath) -> Option<&Self::Mapping>;

    /// Returns the number of Mappings in the interface.
    fn mappings_len(&self) -> usize;
}

/// Astarte interface implementation.
///
/// Should be used only through its conversion methods, not instantiated directly.
#[derive(Debug, PartialEq, Eq, Clone, Deserialize)]
#[serde(try_from = "InterfaceJson<std::borrow::Cow<str>>")]
pub struct Interface {
    inner: InterfaceTypeAggregation,
}

impl Interface {
    /// Returns the interface name.
    pub fn interface_name(&self) -> &str {
        match &self.inner {
            InterfaceTypeAggregation::DatastreamIndividual(datastream_individual) => {
                datastream_individual.interface_name()
            }
            InterfaceTypeAggregation::DatastreamObject(datastream_object) => {
                datastream_object.interface_name()
            }
            InterfaceTypeAggregation::Property(property) => property.interface_name(),
        }
    }

    /// Returns the interface major version.
    pub fn version_major(&self) -> i32 {
        match &self.inner {
            InterfaceTypeAggregation::DatastreamIndividual(datastream_individual) => {
                datastream_individual.version_major()
            }
            InterfaceTypeAggregation::DatastreamObject(datastream_object) => {
                datastream_object.version_major()
            }
            InterfaceTypeAggregation::Property(property) => property.version_major(),
        }
    }

    /// Returns the interface minor version.
    pub fn version_minor(&self) -> i32 {
        match &self.inner {
            InterfaceTypeAggregation::DatastreamIndividual(datastream_individual) => {
                datastream_individual.version_minor()
            }
            InterfaceTypeAggregation::DatastreamObject(datastream_object) => {
                datastream_object.version_minor()
            }
            InterfaceTypeAggregation::Property(property) => property.version_minor(),
        }
    }

    /// Returns the interface type.
    pub fn interface_type(&self) -> InterfaceType {
        match &self.inner {
            InterfaceTypeAggregation::DatastreamIndividual(_)
            | InterfaceTypeAggregation::DatastreamObject(_) => InterfaceType::Datastream,
            InterfaceTypeAggregation::Property(_) => InterfaceType::Properties,
        }
    }

    /// Returns the interface ownership.
    pub fn ownership(&self) -> Ownership {
        self.ownership
    }

    /// Returns the interface aggregation.
    pub fn aggregation(&self) -> Aggregation {
        match &self.inner {
            InterfaceTypeAggregation::Property(_)
            | InterfaceTypeAggregation::DatastreamIndividual(_) => Aggregation::Individual,
            InterfaceTypeAggregation::DatastreamObject(_) => Aggregation::Object,
        }
    }

    /// Validate if an interface is given the previous version `prev`.
    ///
    /// It will check whether:
    ///
    /// - Both the versions are valid
    /// - The name of the interface is the same
    /// - The new version is a valid successor of the previous version.
    pub fn validate_with(&self, prev: &Self) -> Result<&Self, Error> {
        // If the interfaces are the same, they are valid
        if self == prev {
            return Ok(self);
        }

        // Check if the wrong interface was passed
        let name = self.interface_name();
        let prev_name = prev.interface_name();
        if name != prev_name {
            return Err(Error::NameMismatch {
                name: name.to_string(),
                prev_name: prev_name.to_string(),
            });
        }

        // Validate the new interface version
        VersionChange::try_new(self, prev)
            .map_err(Error::VersionChange)
            .map(|change| {
                info!("Interface {} version changed: {}", name, change);

                self
            })
    }
}

impl Serialize for Interface {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let interface_def = InterfaceJson::from(self);

        interface_def.serialize(serializer)
    }
}

impl FromStr for Interface {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s).map_err(Self::Err::from)
    }
}

impl Display for Interface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.interface_name, self.version)
    }
}

/// Enum of all the types and aggregation of interfaces
///
/// This is not a direct representation of only the mapping to permit extensibility of specific
/// properties present only in some aggregations.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InterfaceTypeAggregation {
    /// Interface with type datastream and aggregations individual.
    DatastreamIndividual(DatastreamIndividual),
    /// Interface with type datastream and aggregations object.
    DatastreamObject(DatastreamObject),
    /// A property interface.
    Property(Property),
}

impl InterfaceTypeAggregation {
    /// Return a reference to a [`DatastreamIndividual`].
    pub fn as_datastream_individual(&self) -> Option<&DatastreamIndividual> {
        if let Self::DatastreamIndividual(v) = self {
            Some(v)
        } else {
            None
        }
    }

    /// Return a reference to a [`DatastreamObject`].
    pub fn as_datastream_object(&self) -> Option<&DatastreamObject> {
        if let Self::DatastreamObject(v) = self {
            Some(v)
        } else {
            None
        }
    }

    /// Return a reference to a [`Property`].
    pub fn as_property(&self) -> Option<&Property> {
        if let Self::Property(v) = self {
            Some(v)
        } else {
            None
        }
    }

    /// Returns `true` if the interface type is [`DatastreamIndividual`].
    ///
    /// [`DatastreamIndividual`]: InterfaceType::DatastreamIndividual
    #[must_use]
    pub fn is_datastream_individual(&self) -> bool {
        matches!(self, Self::DatastreamIndividual(..))
    }

    /// Returns `true` if the interface type is [`DatastreamObject`].
    ///
    /// [`DatastreamObject`]: InterfaceType::DatastreamObject
    #[must_use]
    pub fn is_datastream_object(&self) -> bool {
        matches!(self, Self::DatastreamObject(..))
    }

    /// Returns `true` if the interface type is [`Properties`].
    ///
    /// [`Properties`]: InterfaceType::Properties
    #[must_use]
    pub fn is_property(&self) -> bool {
        matches!(self, Self::Property(..))
    }
}

/// Defines the retention of a data stream.
///
/// Describes what to do with the sent data if the transport is incapable of delivering it.
///
/// See [Retention](https://docs.astarte-platform.org/astarte/latest/040-interface_schema.html#astarte-mapping-schema-retention)
/// for more information.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Retention {
    /// Data is discarded.
    Discard,
    /// Data is kept in a cache in memory.
    Volatile {
        /// Duration for the data to expire.
        ///
        /// If it's [`None`] it will never expire.
        expiry: Option<Duration>,
    },
    /// Data is kept on disk.
    Stored {
        /// Duration for the data to expire.
        ///
        /// If it's [`None`] it will never expire.
        expiry: Option<Duration>,
    },
}

impl Retention {
    /// Returns `true` if the retention is [`Stored`].
    ///
    /// [`Stored`]: Retention::Stored
    #[must_use]
    pub const fn is_stored(&self) -> bool {
        matches!(self, Self::Stored { .. })
    }

    /// Returns the expiry for the retention.
    ///
    /// For the [`Discard`](Retention::Discard) will always return [`None`], while for
    /// [`Volatile`](Retention::Volatile) or [`Stored`](Retention::Stored) returns the inner expiry
    /// only if set.
    #[must_use]
    pub const fn as_expiry(&self) -> Option<&Duration> {
        match self {
            Retention::Discard => None,
            // Duration is copy
            Retention::Volatile { expiry } => expiry.as_ref(),
            Retention::Stored { expiry } => expiry.as_ref(),
        }
    }

    /// Returns `true` if the retention is [`Volatile`].
    ///
    /// [`Volatile`]: Retention::Volatile
    #[must_use]
    pub const fn is_volatile(&self) -> bool {
        matches!(self, Self::Volatile { .. })
    }

    /// Returns `true` if the retention is [`Discard`].
    ///
    /// [`Discard`]: Retention::Discard
    #[must_use]
    pub const fn is_discard(&self) -> bool {
        matches!(self, Self::Discard)
    }
}

impl Default for Retention {
    fn default() -> Self {
        Self::Discard
    }
}

/// Defines if data should be expired from the database after a given interval.
///
/// See [Database Retention Policy](https://docs.astarte-platform.org/astarte/latest/040-interface_schema.html#astarte-mapping-schema-database_retention_policy)
/// for more information.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DatabaseRetention {
    /// Data will never expire.
    NoTtl,
    /// Data will live for the ttl.
    UseTtl {
        /// Time to live int the database.
        ttl: Duration,
    },
}

impl DatabaseRetention {
    /// Returns `true` if the database retention is [`NoTtl`].
    ///
    /// [`NoTtl`]: DatabaseRetention::NoTtl
    #[must_use]
    pub fn is_no_ttl(&self) -> bool {
        matches!(self, Self::NoTtl)
    }

    /// Returns `true` if the database retention is [`UseTtl`].
    ///
    /// [`UseTtl`]: DatabaseRetention::UseTtl
    #[must_use]
    pub fn is_use_ttl(&self) -> bool {
        matches!(self, Self::UseTtl { .. })
    }

    pub fn as_ttl(&self) -> Option<&Duration> {
        match self {
            DatabaseRetention::NoTtl => None,
            DatabaseRetention::UseTtl { ttl } => Some(ttl),
        }
    }
}

impl Default for DatabaseRetention {
    fn default() -> Self {
        Self::NoTtl
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::{
        interface::{
            def::{DatabaseRetentionPolicyDef, MappingType, RetentionDef},
            Aggregation, DatabaseRetention, DatastreamIndividual, InterfaceType,
            InterfaceTypeAggregation, Mapping, MappingAccess, MappingSet, Ownership, Reliability,
            Retention,
        },
        mapping::{
            path::MappingPath,
            vec::{Item, MappingVec},
            BaseMapping, DatastreamIndividualMapping,
        },
        test::{E2E_DEVICE_AGGREGATE, E2E_DEVICE_DATASTREAM, E2E_DEVICE_PROPERTY},
        Interface,
    };

    // The mappings are sorted alphabetically by endpoint, so we can confront them
    #[cfg(feature = "interface-doc")]
    const INTERFACE_JSON: &str = r#"{
            "interface_name": "org.astarte-platform.genericsensors.Values",
            "version_major": 1,
            "version_minor": 0,
            "type": "datastream",
            "ownership": "device",
            "description": "Interface description",
            "doc": "Interface doc",
            "mappings": [
                {
                    "endpoint": "/%{sensor_id}/otherValue",
                    "type": "longinteger",
                    "explicit_timestamp": true,
                    "description": "Mapping description",
                    "doc": "Mapping doc"
                },
                {
                    "endpoint": "/%{sensor_id}/value",
                    "type": "double",
                    "explicit_timestamp": true,
                    "description": "Mapping description",
                    "doc": "Mapping doc"
                }
            ]
        }"#;

    #[cfg(not(feature = "interface-doc"))]
    const INTERFACE_JSON: &str = r#"{
            "interface_name": "org.astarte-platform.genericsensors.Values",
            "version_major": 1,
            "version_minor": 0,
            "type": "datastream",
            "ownership": "device",
            "mappings": [
                {
                    "endpoint": "/%{sensor_id}/otherValue",
                    "type": "longinteger",
                    "explicit_timestamp": true
                },
                {
                    "endpoint": "/%{sensor_id}/value",
                    "type": "double",
                    "explicit_timestamp": true
                }
            ]
        }"#;

    // The mappings are sorted alphabetically by endpoint, so we can confront them
    const PROPERTIES_JSON: &str = r#"{
            "interface_name": "org.astarte-platform.genericproperties.Values",
            "version_major": 1,
            "version_minor": 0,
            "type": "properties",
            "ownership": "server",
            "description": "Interface description",
            "doc": "Interface doc",
            "mappings": [
                {
                    "endpoint": "/%{sensor_id}/aaaa",
                    "type": "longinteger",
                    "allow_unset": true
                },
                {
                    "endpoint": "/%{sensor_id}/bbbb",
                    "type": "double",
                    "allow_unset": false
                }
            ]
        }"#;

    #[test]
    fn datastream_interface_deserialization() {
        let value_mapping = DatastreamIndividualMapping {
            mapping: BaseMapping {
                endpoint: "/%{sensor_id}/value".try_into().unwrap(),
                mapping_type: MappingType::Double,
                #[cfg(feature = "interface-doc")]
                description: Some("Mapping description".to_string()),
                #[cfg(feature = "interface-doc")]
                doc: Some("Mapping doc".to_string()),
            },
            reliability: Reliability::default(),
            retention: Retention::default(),
            database_retention: DatabaseRetention::default(),
            explicit_timestamp: true,
        };

        let other_value_mapping = DatastreamIndividualMapping {
            mapping: BaseMapping {
                endpoint: "/%{sensor_id}/otherValue".try_into().unwrap(),
                mapping_type: MappingType::LongInteger,
                #[cfg(feature = "interface-doc")]
                description: Some("Mapping description".to_string()),
                #[cfg(feature = "interface-doc")]
                doc: Some("Mapping doc".to_string()),
            },
            reliability: Reliability::default(),
            retention: Retention::default(),
            database_retention: DatabaseRetention::default(),
            explicit_timestamp: true,
        };

        let interface_name = "org.astarte-platform.genericsensors.Values".to_owned();
        let version_major = 1;
        let version_minor = 0;
        let ownership = Ownership::Device;
        #[cfg(feature = "interface-doc")]
        let description = Some("Interface description".to_owned());
        #[cfg(feature = "interface-doc")]
        let doc = Some("Interface doc".to_owned());

        let btree = MappingSet::from_iter(
            [value_mapping, other_value_mapping]
                .into_iter()
                .map(Item::new),
        );

        let datastream_individual = DatastreamIndividual {
            mappings: MappingVec::from(btree),
        };

        let interface = Interface {
            interface_name,
            version_major,
            version_minor,
            ownership,
            #[cfg(feature = "interface-doc")]
            description,
            #[cfg(feature = "interface-doc")]
            doc,
            inner: InterfaceTypeAggregation::DatastreamIndividual(datastream_individual),
        };

        let deser_interface = Interface::from_str(INTERFACE_JSON).unwrap();

        assert_eq!(interface, deser_interface);
    }

    #[test]
    fn must_have_one_mapping() {
        let json = r#"{
            "interface_name": "org.astarte-platform.genericproperties.Values",
            "version_major": 1,
            "version_minor": 0,
            "type": "properties",
            "ownership": "server",
            "description": "Interface description",
            "doc": "Interface doc",
            "mappings": []
        }"#;

        let interface = Interface::from_str(json);

        assert!(interface.is_err());
        // This is hacky but serde doesn't provide a way to check the error
        let err = format!("{:?}", interface.unwrap_err());
        assert!(err.contains("no mappings"), "Unexpected error: {}", err);
    }

    #[test]
    fn test_properties() {
        let interface = Interface::from_str(PROPERTIES_JSON).unwrap();

        assert!(interface.is_property(), "Properties interface not found");
        assert_eq!(interface.version(), (1, 0));
        assert_eq!(interface.version_major(), 1);
        assert_eq!(interface.version_minor(), 0);

        let paths: Vec<_> = interface.iter_mappings().collect();

        assert_eq!(paths.len(), 2);
        assert_eq!(*paths[0].endpoint(), "/%{sensor_id}/aaaa");
        assert_eq!(*paths[1].endpoint(), "/%{sensor_id}/bbbb");

        let path = MappingPath::try_from("/1/aaaa").unwrap();

        let f = interface.mapping(&path).unwrap();

        assert_eq!(f.mapping_type(), MappingType::LongInteger);
        assert!(f.allow_unset);
    }

    #[test]
    fn test_iter_mappings() {
        let value_mapping = Mapping {
            endpoint: "/%{sensor_id}/value",
            mapping_type: MappingType::Double,
            #[cfg(feature = "interface-doc")]
            description: Some("Mapping description"),
            #[cfg(feature = "interface-doc")]
            doc: Some("Mapping doc"),
            #[cfg(not(feature = "interface-doc"))]
            description: (),
            #[cfg(not(feature = "interface-doc"))]
            doc: (),
            reliability: Reliability::default(),
            retention: RetentionDef::default(),
            database_retention_policy: DatabaseRetentionPolicyDef::default(),
            database_retention_ttl: None,
            allow_unset: false,
            expiry: 0,
            explicit_timestamp: true,
        };

        let other_value_mapping = Mapping {
            endpoint: "/%{sensor_id}/otherValue",
            mapping_type: MappingType::LongInteger,
            #[cfg(feature = "interface-doc")]
            description: Some("Mapping description"),
            #[cfg(feature = "interface-doc")]
            doc: Some("Mapping doc"),
            #[cfg(not(feature = "interface-doc"))]
            description: (),
            #[cfg(not(feature = "interface-doc"))]
            doc: (),
            reliability: Reliability::default(),
            retention: RetentionDef::default(),
            database_retention_policy: DatabaseRetentionPolicyDef::default(),
            database_retention_ttl: None,
            allow_unset: false,
            expiry: 0,
            explicit_timestamp: true,
        };

        let interface = Interface::from_str(INTERFACE_JSON).unwrap();

        let mut mappings_iter = interface.iter_mappings();

        assert_eq!(mappings_iter.len(), 2);
        assert_eq!(mappings_iter.next(), Some(other_value_mapping));
        assert_eq!(mappings_iter.next(), Some(value_mapping));
    }

    #[test]
    fn methods_test() {
        let interface = Interface::from_str(INTERFACE_JSON).unwrap();

        assert_eq!(
            interface.interface_name(),
            "org.astarte-platform.genericsensors.Values"
        );
        assert_eq!(interface.version_major(), 1);
        assert_eq!(interface.version_minor(), 0);
        assert_eq!(interface.ownership(), Ownership::Device);
        #[cfg(feature = "interface-doc")]
        assert_eq!(interface.description(), Some("Interface description"));
        assert_eq!(interface.aggregation(), Aggregation::Individual);
        assert_eq!(interface.interface_type(), InterfaceType::Datastream);
        #[cfg(feature = "interface-doc")]
        assert_eq!(interface.doc(), Some("Interface doc"));
    }

    #[test]
    fn serialize_and_deserialize() {
        let interface = Interface::from_str(INTERFACE_JSON).unwrap();
        let serialized = serde_json::to_string(&interface).unwrap();
        let deserialized: Interface = serde_json::from_str(&serialized).unwrap();

        assert_eq!(interface, deserialized);

        let value = serde_json::Value::from_str(&serialized).unwrap();
        let expected = serde_json::Value::from_str(INTERFACE_JSON).unwrap();
        assert_eq!(value, expected);
    }

    #[test]
    fn check_as_prop() {
        let interface = Interface::from_str(PROPERTIES_JSON).unwrap();

        let prop = interface.as_prop().expect("interface is a property");

        assert!(std::ptr::eq(prop.0, &interface));

        let interface = Interface::from_str(INTERFACE_JSON).unwrap();

        assert_eq!(interface.as_prop(), None);
    }

    #[cfg(feature = "interface-doc")]
    #[test]
    fn test_with_escaped_descriptions() {
        let json = r#"{
            "interface_name": "org.astarte-platform.genericproperties.Values",
            "version_major": 1,
            "version_minor": 0,
            "type": "properties",
            "ownership": "server",
            "description": "Interface description \"escaped\"",
            "doc": "Interface doc \"escaped\"",
            "mappings": [{
                "endpoint": "/double_endpoint",
                "type": "double",
                "doc": "Mapping doc \"escaped\""
            }]
        }"#;

        let interface = Interface::from_str(json).unwrap();

        assert_eq!(
            interface.description().unwrap(),
            r#"Interface description "escaped""#
        );
        assert_eq!(interface.doc().unwrap(), r#"Interface doc "escaped""#);
        assert_eq!(
            *interface
                .mapping(&crate::interface::mapping::path::tests::mapping(
                    "/double_endpoint"
                ))
                .unwrap()
                .doc()
                .unwrap(),
            r#"Mapping doc "escaped""#
        );
    }

    #[test]
    fn should_convert_into_inner() {
        let interface = Interface::from_str(E2E_DEVICE_PROPERTY).unwrap();

        assert!(interface.as_inner().as_properties().is_some());
        assert!(interface.as_inner().as_datastream_object().is_none());
        assert!(interface.as_inner().as_datastream_individual().is_none());
        assert!(interface.as_inner().is_properties());
        assert!(!interface.as_inner().is_datastream_object());
        assert!(!interface.as_inner().is_datastream_object());

        let inner = interface.into_inner();
        let interface = inner.as_properties().unwrap();
        assert_eq!(interface.len(), 14);
        assert!(!interface.is_empty());

        let interface = Interface::from_str(E2E_DEVICE_AGGREGATE).unwrap();

        assert!(interface.as_inner().as_properties().is_none());
        assert!(interface.as_inner().as_datastream_object().is_some());
        assert!(interface.as_inner().as_datastream_individual().is_none());
        assert!(!interface.as_inner().is_properties());
        assert!(interface.as_inner().is_datastream_object());
        assert!(!interface.as_inner().is_datastream_individual());

        let inner = interface.into_inner();
        let interface = inner.as_datastream_object().unwrap();
        assert_eq!(interface.len(), 14);
        assert!(!interface.is_empty());

        let interface = Interface::from_str(E2E_DEVICE_DATASTREAM).unwrap();

        assert!(interface.as_inner().as_properties().is_none());
        assert!(interface.as_inner().as_datastream_object().is_none());
        assert!(interface.as_inner().as_datastream_individual().is_some());
        assert!(!interface.as_inner().is_properties());
        assert!(!interface.as_inner().is_datastream_object());
        assert!(interface.as_inner().is_datastream_individual());

        let inner = interface.into_inner();
        let interface = inner.as_datastream_individual().unwrap();
        assert_eq!(interface.len(), 14);
        assert!(!interface.is_empty());
    }
}
