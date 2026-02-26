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

//! Support for UUIDs for Astarte.

use std::fmt::Display;
use std::ops::{Deref, DerefMut};

use tracing::error;
use uuid::Uuid;

use super::{AstarteData, TypeError};

/// A [`uuid`] represented as a binary Astarte value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BinUuid(Uuid);

impl Display for BinUuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Deref for BinUuid {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for BinUuid {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Uuid> for BinUuid {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl From<BinUuid> for Uuid {
    fn from(value: BinUuid) -> Self {
        value.0
    }
}

impl From<BinUuid> for AstarteData {
    fn from(value: BinUuid) -> Self {
        AstarteData::BinaryBlob(value.0.into())
    }
}
impl PartialEq<BinUuid> for AstarteData {
    fn eq(&self, other: &BinUuid) -> bool {
        let AstarteData::BinaryBlob(value) = self else {
            return false;
        };

        value == other.as_bytes()
    }
}
impl PartialEq<AstarteData> for BinUuid {
    fn eq(&self, other: &AstarteData) -> bool {
        other.eq(self)
    }
}

impl TryFrom<AstarteData> for BinUuid {
    type Error = TypeError;

    fn try_from(value: AstarteData) -> Result<Self, Self::Error> {
        let AstarteData::BinaryBlob(value) = value else {
            return Err(Self::Error::conversion(format!(
                "from {} into binary UUID",
                value.display_type()
            )));
        };
        Uuid::try_from(value)
            .map_err(|error| {
                error!(%error,"couldn't parse binary UUID");

                TypeError::Conversion {
                    ctx: "couldn't parse binary UUID".to_string(),
                }
            })
            .map(BinUuid)
    }
}

/// A [`uuid`] represented as a dashed string Astarte value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StrUuid(Uuid);

impl Display for StrUuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Deref for StrUuid {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for StrUuid {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Uuid> for StrUuid {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl From<StrUuid> for Uuid {
    fn from(value: StrUuid) -> Self {
        value.0
    }
}

impl From<StrUuid> for AstarteData {
    fn from(value: StrUuid) -> Self {
        AstarteData::String(value.0.to_string())
    }
}
impl PartialEq<StrUuid> for AstarteData {
    fn eq(&self, other: &StrUuid) -> bool {
        let AstarteData::String(value) = self else {
            return false;
        };

        Uuid::parse_str(value).is_ok_and(|v| v == other.0)
    }
}
impl PartialEq<AstarteData> for StrUuid {
    fn eq(&self, other: &AstarteData) -> bool {
        other.eq(self)
    }
}

impl TryFrom<AstarteData> for StrUuid {
    type Error = TypeError;

    fn try_from(value: AstarteData) -> Result<Self, Self::Error> {
        let AstarteData::String(value) = value else {
            return Err(Self::Error::conversion(format!(
                "from {} into binary UUID",
                value.display_type()
            )));
        };

        Uuid::parse_str(&value)
            .map_err(|error| {
                error!(%error,"couldn't parse string UUID");

                TypeError::Conversion {
                    ctx: "couldn't parse string UUID".to_string(),
                }
            })
            .map(StrUuid)
    }
}
