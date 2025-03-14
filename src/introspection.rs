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

//! Handle the introspection for the device

use std::{
    future::Future,
    path::{Path, PathBuf},
};

use tracing::{debug, error};

use crate::{interface::error::InterfaceError, Error, Interface};

/// Error while adding an [`Interface`] to the device introspection.
#[non_exhaustive]
#[derive(thiserror::Error, Debug)]
pub enum AddInterfaceError {
    /// Couldn't add the interface.
    #[error("error adding interface")]
    Interface(#[from] InterfaceError),
    /// Failed to read interface directory.
    #[error("couldn't read interface path {}", .path.display())]
    Io {
        /// The path of the interface json file we couldn't read.
        path: PathBuf,
        #[source]
        /// The IO error.
        backtrace: std::io::Error,
    },
    /// Cannot read the interface file.
    #[error("invalid interface file {}", .path.display())]
    InterfaceFile {
        /// The path of the invalid interface json.
        path: PathBuf,
        /// Reason why the interface couldn't be added.
        #[source]
        backtrace: InterfaceError,
    },
}

impl AddInterfaceError {
    // Add a path to the error context.
    pub(crate) fn add_path_context(self, path: PathBuf) -> Self {
        match self {
            AddInterfaceError::Interface(backtrace) => {
                AddInterfaceError::InterfaceFile { path, backtrace }
            }
            AddInterfaceError::Io {
                path: prev,
                backtrace,
            } => {
                debug!("overwriting previous path {}", prev.display());

                AddInterfaceError::Io { path, backtrace }
            }
            AddInterfaceError::InterfaceFile {
                path: prev,
                backtrace,
            } => {
                debug!("overwriting previous path {}", prev.display());

                AddInterfaceError::InterfaceFile { path, backtrace }
            }
        }
    }
}

/// Trait that permits a client to query the interfaces in the device introspection.
pub trait DeviceIntrospection {
    /// Returns a reference to the [`Interface`] with the given name.
    fn get_interface<F, O>(&self, interface_name: &str, f: F) -> impl Future<Output = O> + Send
    where
        F: FnMut(Option<&Interface>) -> O + Send;
}

/// Trait that permits a client to add and remove interfaces dynamically after being connected.
pub trait DynamicIntrospection {
    /// Add a new [`Interface`] to the device introspection.
    ///
    /// Returns a bool to check weather the if the interface was added or was already present.
    fn add_interface(
        &self,
        interface: Interface,
    ) -> impl Future<Output = Result<bool, Error>> + Send;

    /// Add one or more [`Interface`] to the device introspection.
    ///
    /// Returns a [`Vec`] with the name of the interfaces that have been added.
    fn extend_interfaces<I>(
        &self,
        interfaces: I,
    ) -> impl Future<Output = Result<Vec<String>, Error>> + Send
    where
        I: IntoIterator<Item = Interface> + Send;

    /// Add one or more [`Interface`] to the device introspection, specialized for a [`Vec`].
    ///
    /// Returns a [`Vec`] with the name of the interfaces that have been added.
    fn extend_interfaces_vec(
        &self,
        interfaces: Vec<Interface>,
    ) -> impl Future<Output = Result<Vec<String>, Error>> + Send;

    /// Add a new interface from the provided file.
    ///
    /// Returns a bool to check weather the if the interface was added or was already present.
    fn add_interface_from_file<P>(
        &self,
        file_path: P,
    ) -> impl Future<Output = Result<bool, Error>> + Send
    where
        P: AsRef<Path> + Send + Sync;

    /// Add a new interface from a string. The string should contain a valid json formatted
    /// interface.
    ///
    /// Returns a bool to check weather the if the interface was added or was already present.
    fn add_interface_from_str(
        &self,
        json_str: &str,
    ) -> impl Future<Output = Result<bool, Error>> + Send;

    /// Remove the interface with the name specified as argument.
    ///
    /// Returns a bool to check weather the if the interface was removed or was missing.
    fn remove_interface(
        &self,
        interface_name: &str,
    ) -> impl Future<Output = Result<bool, Error>> + Send;

    /// Remove interfaces with names specified as argument.
    ///
    /// Returns a [`Vec`] with the name of the interfaces that have been removed.
    fn remove_interfaces<I>(
        &self,
        interfaces_name: I,
    ) -> impl Future<Output = Result<Vec<String>, Error>> + Send
    where
        I: IntoIterator<Item = String> + Send,
        I::IntoIter: Send;

    /// Remove one or more [`Interface`] from the device introspection, specialized for a [`Vec`].
    ///
    /// Returns a [`Vec`] with the name of the interfaces that have been added.
    fn remove_interfaces_vec(
        &self,
        interfaces_name: Vec<String>,
    ) -> impl Future<Output = Result<Vec<String>, Error>> + Send;
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::str::FromStr;

    use itertools::Itertools;
    use mockall::predicate;
    use rumqttc::{AckOfPub, SubAck, SubscribeFilter, UnsubAck};

    use super::*;

    use crate::interfaces::Introspection;
    use crate::test::{
        mock_astarte_device, E2E_DEVICE_AGGREGATE, E2E_DEVICE_DATASTREAM, E2E_DEVICE_PROPERTY,
    };
    use crate::transport::mqtt::client::{AsyncClient, EventLoop as MqttEventLoop};
    use crate::transport::mqtt::test::notify_success;

    #[tokio::test]
    async fn test_add_remove_interface() {
        let eventloop = MqttEventLoop::default();
        let mut client = AsyncClient::default();

        let mut seq = mockall::Sequence::new();

        client
            .expect_clone()
            .once()
            .in_sequence(&mut seq)
            .returning(AsyncClient::default);

        client
            .expect_subscribe::<String>()
            .once()
            .in_sequence(&mut seq)
            .with(
                predicate::eq("realm/device_id/org.astarte-platform.rust.examples.individual-datastream.ServerDatastream/#".to_string()),
                predicate::always()
            )
            .returning(|_, _| notify_success(SubAck::new(0,Vec::new())));

        client
            .expect_publish::<String, String>()
            .once()
            .in_sequence(&mut seq)
            .with(
                predicate::eq("realm/device_id".to_string()),
                predicate::always(),
                predicate::eq(false),
                predicate::eq(
                    "org.astarte-platform.rust.examples.individual-datastream.ServerDatastream:0:1"
                        .to_string(),
                ),
            )
            .returning(|_, _, _, _| notify_success(AckOfPub::None));

        client
            .expect_publish::<String, String>()
            .with(
                predicate::eq("realm/device_id".to_string()),
                predicate::always(),
                predicate::eq(false),
                predicate::eq(String::new()),
            )
            .returning(|_, _, _, _| notify_success(AckOfPub::None));

        client
            .expect_unsubscribe::<String>()
            .with(predicate::eq("realm/device_id/org.astarte-platform.rust.examples.individual-datastream.ServerDatastream/#".to_string()))
            .returning(|_|notify_success(UnsubAck::new(0)));

        let (client, mut connection) = mock_astarte_device(client, eventloop, []).await;

        let handle = tokio::spawn(async move {
            for _ in 0..3 {
                let msg = connection.sender.client.recv().await.unwrap();
                connection.sender.handle_client_msg(msg).await.unwrap();
            }
        });

        let res = client
            .add_interface_from_str(crate::test::INDIVIDUAL_SERVER_DATASTREAM)
            .await
            .unwrap();
        assert!(res);

        // Shouldn't add the second one
        let res = client
            .add_interface_from_str(crate::test::INDIVIDUAL_SERVER_DATASTREAM)
            .await
            .unwrap();
        assert!(!res);

        let res = client
            .remove_interface(
                "org.astarte-platform.rust.examples.individual-datastream.ServerDatastream",
            )
            .await
            .unwrap();
        assert!(res);

        handle.await.unwrap();
    }

    #[tokio::test]
    async fn should_extend_interfaces() {
        let eventloop = MqttEventLoop::default();
        let mut client = AsyncClient::default();

        let i1 = Interface::from_str(crate::test::DEVICE_PROPERTIES).unwrap();
        let i2 = Interface::from_str(crate::test::OBJECT_DEVICE_DATASTREAM).unwrap();
        let i3 = Interface::from_str(crate::test::INDIVIDUAL_SERVER_DATASTREAM).unwrap();
        let i4 = Interface::from_str(crate::test::SERVER_PROPERTIES).unwrap();

        let to_add = [i1.clone(), i2.clone(), i3.clone(), i4.clone()];

        let mut names = to_add.clone().map(|i| i.interface_name().to_string());
        names.sort();

        let mut introspection = Introspection::new(to_add.iter())
            .to_string()
            .split(';')
            .map(ToOwned::to_owned)
            .collect_vec();
        introspection.sort_unstable();

        let mut seq = mockall::Sequence::new();
        client
            .expect_clone()
            .once()
            .in_sequence(&mut seq)
            .returning(AsyncClient::default);

        client
            .expect_subscribe_many::<Vec<SubscribeFilter>>()
            .once()
            .in_sequence(&mut seq)
            .returning(|_| notify_success(SubAck::new(0, Vec::new())));

        let introspection_cpy = introspection.clone();

        client
            .expect_publish::<String, String>()
            .once()
            .in_sequence(&mut seq)
            .withf(move |publish, _, _, payload| {
                let mut intro = payload.split(';').collect_vec();

                intro.sort_unstable();

                publish == "realm/device_id" && intro == introspection_cpy
            })
            .returning(|_, _, _, _| notify_success(AckOfPub::None));

        let (client, mut connection) = mock_astarte_device(client, eventloop, []).await;

        let handle = tokio::spawn(async move {
            let msg = connection.sender.client.recv().await.unwrap();
            connection.sender.handle_client_msg(msg).await.unwrap();
        });

        let mut res = client.extend_interfaces(to_add.clone()).await.unwrap();
        res.sort();
        assert_eq!(res, names);

        handle.await.unwrap();
    }

    #[tokio::test]
    async fn should_remove_interfaces() {
        let eventloop = MqttEventLoop::default();
        let mut client = AsyncClient::default();

        let i1 = Interface::from_str(crate::test::DEVICE_PROPERTIES).unwrap();
        let i2 = Interface::from_str(crate::test::OBJECT_DEVICE_DATASTREAM).unwrap();
        let i3 = Interface::from_str(crate::test::INDIVIDUAL_SERVER_DATASTREAM).unwrap();
        let i4 = Interface::from_str(crate::test::SERVER_PROPERTIES).unwrap();

        let interfaces = [i1.clone(), i2.clone(), i3.clone(), i4.clone()];
        let to_remove = [
            i1.interface_name(),
            i2.interface_name(),
            i3.interface_name(),
            i4.interface_name(),
        ]
        .map(|i| i.to_string());

        let mut names = to_remove.clone();
        names.sort();

        let mut seq = mockall::Sequence::new();

        client
            .expect_clone()
            .once()
            .in_sequence(&mut seq)
            .returning(AsyncClient::default);

        client
            .expect_publish::<String, String>()
            .once()
            .in_sequence(&mut seq)
            .with(
                predicate::eq("realm/device_id".to_string()),
                predicate::always(),
                predicate::eq(false),
                predicate::eq(String::new()),
            )
            .returning(|_, _, _, _| notify_success(AckOfPub::None));

        client
            .expect_unsubscribe::<String>()
            // 2 times since only 2 out of 4 interfaces are server-owned
            .times(2)
            .in_sequence(&mut seq)
            .returning(|_| notify_success(UnsubAck::new(0)));

        let (client, mut connection) = mock_astarte_device(client, eventloop, interfaces).await;

        let handle = tokio::spawn(async move {
            let msg = connection.sender.client.recv().await.unwrap();
            connection.sender.handle_client_msg(msg).await.unwrap();
        });

        let mut res = client.remove_interfaces(to_remove).await.unwrap();
        res.sort();
        assert_eq!(res, names);

        handle.await.unwrap();
    }

    #[tokio::test]
    async fn should_extend_and_remove_interfaces() {
        let eventloop = MqttEventLoop::default();
        let mut client = AsyncClient::default();

        let i1 = Interface::from_str(crate::test::DEVICE_PROPERTIES).unwrap();
        let i2 = Interface::from_str(crate::test::OBJECT_DEVICE_DATASTREAM).unwrap();
        let i3 = Interface::from_str(crate::test::INDIVIDUAL_SERVER_DATASTREAM).unwrap();
        let i4 = Interface::from_str(crate::test::SERVER_PROPERTIES).unwrap();

        let to_add = [i1.clone(), i2.clone(), i3.clone(), i4.clone()];
        let to_remove = [
            i1.interface_name(),
            i2.interface_name(),
            i3.interface_name(),
            i4.interface_name(),
        ]
        .map(|i| i.to_string());

        let mut names = to_remove.clone();
        names.sort();

        let mut introspection = Introspection::new(to_add.iter())
            .to_string()
            .split(';')
            .map(ToOwned::to_owned)
            .collect_vec();

        introspection.sort_unstable();

        let mut seq = mockall::Sequence::new();
        client
            .expect_clone()
            .once()
            .in_sequence(&mut seq)
            .returning(AsyncClient::default);

        client
            .expect_subscribe_many::<Vec<SubscribeFilter>>()
            .once()
            .in_sequence(&mut seq)
            .returning(|_| notify_success(SubAck::new(0, Vec::new())));

        client
            .expect_publish::<String, String>()
            .once()
            .in_sequence(&mut seq)
            .withf(move |publish, _, _, payload| {
                let mut intro = payload.split(';').collect_vec();

                intro.sort_unstable();

                publish == "realm/device_id" && intro == introspection
            })
            .returning(|_, _, _, _| notify_success(AckOfPub::None));

        client
            .expect_publish::<String, String>()
            .once()
            .in_sequence(&mut seq)
            .with(
                predicate::eq("realm/device_id".to_string()),
                predicate::always(),
                predicate::eq(false),
                predicate::eq(String::new()),
            )
            .returning(|_, _, _, _| notify_success(AckOfPub::None));

        client
            .expect_unsubscribe::<String>()
            // 2 times since only 2 out of 4 interfaces are server-owned
            .times(2)
            .in_sequence(&mut seq)
            .returning(|_| notify_success(UnsubAck::new(0)));

        let (client, mut connection) = mock_astarte_device(client, eventloop, []).await;

        let handle = tokio::spawn(async move {
            for _ in 0..2 {
                let msg = connection.sender.client.recv().await.unwrap();
                connection.sender.handle_client_msg(msg).await.unwrap();
            }
        });

        let mut res = client.extend_interfaces(to_add.clone()).await.unwrap();
        res.sort();
        assert_eq!(res, names);

        let mut res = client.remove_interfaces(to_remove).await.unwrap();
        res.sort();
        assert_eq!(res, names);

        handle.await.unwrap();
    }

    #[tokio::test]
    async fn should_not_extend_interfaces() {
        let eventloop = MqttEventLoop::default();
        let mut client = AsyncClient::default();

        client.expect_clone().once().returning(AsyncClient::default);

        let to_add = [
            Interface::from_str(crate::test::DEVICE_PROPERTIES).unwrap(),
            Interface::from_str(crate::test::OBJECT_DEVICE_DATASTREAM).unwrap(),
            Interface::from_str(crate::test::INDIVIDUAL_SERVER_DATASTREAM).unwrap(),
        ];

        // no expectations since no interfaces will be added

        // trying to add the already-present interfaces
        let (client, mut connection) = mock_astarte_device(client, eventloop, to_add.clone()).await;

        let handle = tokio::spawn(async move {
            let msg = connection.sender.client.recv().await.unwrap();
            connection.sender.handle_client_msg(msg).await.unwrap();
        });

        let res = client.extend_interfaces(to_add).await.unwrap();
        assert!(res.is_empty());

        handle.await.unwrap();
    }

    #[tokio::test]
    async fn should_not_unsubscribe_interfaces() {
        let eventloop = MqttEventLoop::default();
        let mut client = AsyncClient::default();

        // don't add server-owned properties, thus no unsubscribe should be called
        let i1 = Interface::from_str(crate::test::DEVICE_PROPERTIES).unwrap();
        let i2 = Interface::from_str(crate::test::OBJECT_DEVICE_DATASTREAM).unwrap();

        let to_add = [i1.clone(), i2.clone()];
        let to_remove = [i1.interface_name(), i2.interface_name()].map(|i| i.to_string());

        let mut names = to_remove.clone();
        names.sort();

        let mut introspection = Introspection::new(to_add.iter())
            .to_string()
            .split(';')
            .map(ToOwned::to_owned)
            .collect_vec();

        introspection.sort_unstable();

        // No subscribe many is expected since no server-owned interfaces are added

        let mut seq = mockall::Sequence::new();

        client
            .expect_clone()
            .once()
            .in_sequence(&mut seq)
            .returning(AsyncClient::default);

        client
            .expect_publish::<String, String>()
            .once()
            .in_sequence(&mut seq)
            .returning(|_, _, _, _| notify_success(AckOfPub::None));

        client
            .expect_publish::<String, String>()
            .once()
            .in_sequence(&mut seq)
            .with(
                predicate::eq("realm/device_id".to_string()),
                predicate::always(),
                predicate::eq(false),
                predicate::eq(String::new()),
            )
            .returning(|_, _, _, _| notify_success(AckOfPub::None));

        // no unsubscribe is called since no server-owned interfaces have been added

        let (client, mut connection) = mock_astarte_device(client, eventloop, []).await;

        let handle = tokio::spawn(async move {
            for _ in 0..2 {
                let msg = connection.sender.client.recv().await.unwrap();
                connection.sender.handle_client_msg(msg).await.unwrap();
            }
        });

        let mut res = client.extend_interfaces(to_add.clone()).await.unwrap();
        res.sort();
        assert_eq!(res, names);

        let mut res = client.remove_interfaces(to_remove).await.unwrap();
        res.sort();
        assert_eq!(res, names);

        handle.await.unwrap();
    }

    #[tokio::test]
    async fn remove_non_existing_interfaces() {
        let eventloop = MqttEventLoop::default();
        let mut client = AsyncClient::default();

        let mut seq = mockall::Sequence::new();

        client
            .expect_clone()
            .once()
            .in_sequence(&mut seq)
            .returning(AsyncClient::default);

        let (client, mut connection) = mock_astarte_device(client, eventloop, []).await;

        let handle = tokio::spawn(async move {
            for _ in 0..2 {
                let msg = connection.sender.client.recv().await.unwrap();
                connection.sender.handle_client_msg(msg).await.unwrap();
            }
        });

        let res = client
            .remove_interface("com.example.NonExistingInterface")
            .await
            .unwrap();

        assert!(!res);

        let res = client
            .remove_interfaces(
                [
                    "com.example.NonExistingInterface1",
                    "com.example.NonExistingInterface2",
                ]
                .map(|i| i.to_string()),
            )
            .await
            .unwrap();
        assert!(res.is_empty());

        handle.await.unwrap();
    }

    #[test]
    fn should_add_context_to_err() {
        let ctx = Path::new("/foo/bar");

        let original = AddInterfaceError::Interface(InterfaceError::MajorMinor);
        let err = original.add_path_context(ctx.to_owned());
        assert!(matches!(
            err,
            AddInterfaceError::InterfaceFile { path, backtrace: InterfaceError::MajorMinor } if path == ctx
        ));

        let original = AddInterfaceError::Io {
            path: Path::new("/baz").to_owned(),
            backtrace: io::Error::new(io::ErrorKind::NotFound, "foo"),
        };
        let err = original.add_path_context(ctx.to_owned());
        assert!(matches!(
            err,
            AddInterfaceError::Io { path, backtrace: _ } if path == ctx
        ));

        let original = AddInterfaceError::InterfaceFile {
            path: Path::new("/baz").to_owned(),
            backtrace: InterfaceError::MajorMinor,
        };
        let err = original.add_path_context(ctx.to_owned());
        assert!(matches!(
            err,
            AddInterfaceError::InterfaceFile { path, backtrace: InterfaceError::MajorMinor } if path == ctx
        ));
    }

    #[tokio::test]
    async fn should_get_interface() {
        let eventloop = MqttEventLoop::default();
        let mut client = AsyncClient::default();

        client.expect_clone().once().returning(AsyncClient::default);

        let (device, _) = mock_astarte_device(
            client,
            eventloop,
            [
                Interface::from_str(E2E_DEVICE_PROPERTY).unwrap(),
                Interface::from_str(E2E_DEVICE_AGGREGATE).unwrap(),
                Interface::from_str(E2E_DEVICE_DATASTREAM).unwrap(),
            ],
        )
        .await;

        let interface = device
            .get_interface("org.astarte-platform.rust.e2etest.DeviceProperty", |i| {
                i.cloned()
            })
            .await
            .unwrap();

        assert_eq!(
            interface.interface_name(),
            "org.astarte-platform.rust.e2etest.DeviceProperty"
        );

        let interface = device.get_interface("foo.bar", |i| i.cloned()).await;

        assert_eq!(interface, None);
    }
}
