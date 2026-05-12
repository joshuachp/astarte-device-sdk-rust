// This file is part of Astarte.
//
// Copyright 2026 SECO Mind Srl
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

use std::fmt::Display;

use super::convert::MessageHubProtoError;

/// Errors raised while using the [`Grpc`](super::Grpc) transport
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NewGrpcError {
    /// The gRPC connection returned an error.
    Transport,
    /// Status code error.
    Status,
    /// Couldn't serialize interface to json.
    InterfacesSerialization,
    /// Failed to convert a proto message.
    Conversion(MessageHubProtoError),
    /// Error returned by the message hub server
    Server,
}

impl Display for NewGrpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NewGrpcError::Transport => write!(f, "transport error"),
            NewGrpcError::Status => write!(f, "error status code"),
            NewGrpcError::InterfacesSerialization => write!(f, "error serializing the interface"),
            NewGrpcError::Conversion(error) => write!(f, "conversion error {error}"),
            NewGrpcError::Server => write!(f, "server error message"),
        }
    }
}
