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

use coset::CoseKey;

#[derive(Debug, Clone, PartialEq, minicbor::Encode, minicbor::Decode)]
pub(crate) struct InitExchange {
    #[n(0)]
    pub(crate) seq: u16,
    #[n(1)]
    pub(crate) alg: Alg,
    #[cbor(n(2), with = "super::derive::cose_key")]
    pub(crate) pub_key: CoseKey,
    #[n(3)]
    pub(crate) hkdf_salt: [u8; 32],
}

impl Display for InitExchange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let InitExchange {
            seq,
            alg,
            pub_key,
            hkdf_salt,
        } = self;

        // TODO: should we hex encode the salt?
        write!(
            f,
            "InitExchange [ {seq}, {alg}, {pub_key:?}, {hkdf_salt:?} ]"
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, minicbor::Encode, minicbor::Decode)]
#[cbor(index_only)]
pub(crate) enum Alg {
    #[n(0)]
    EcdhP256HkdfSha256Aes256Gcm = 0,
    #[n(1)]
    EcdhX25519HkdfSha256Aes256Gcm = 1,
}

impl std::fmt::Display for Alg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Alg::EcdhX25519HkdfSha256Aes256Gcm => {
                write!(f, "ECDH_X25518-HKDF_SHA256-AES_256_GCM")
            }
            Alg::EcdhP256HkdfSha256Aes256Gcm => {
                write!(f, "ECDH_P256-HKDF_SHA256-AES_256_GCM")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, minicbor::Encode, minicbor::Decode)]
pub(crate) struct ExchangeResp {
    #[n(0)]
    seq: u16,
    #[cbor(n(1), with = "super::derive::cose_key")]
    pub_key: CoseKey,
}

impl Display for ExchangeResp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ExchangeResp { seq, pub_key } = self;
        write!(f, "ExchangeResp [ {seq}, {pub_key:?} ]")
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use astarte_test_utils::{Hexdump, with_insta};
    use coset::CoseKeyBuilder;
    use insta::assert_snapshot;

    use super::*;

    pub(crate) const PUB_ECC_KEY_PARAMS: &str =
        include_str!("../../../../assets/test-keys/ec-pub-key.params.hex");

    pub(crate) fn cose_key() -> CoseKey {
        let (x, y) = ecc_p256_params();

        CoseKeyBuilder::new_ec2_pub_key(coset::iana::EllipticCurve::P_256, x.to_vec(), y.to_vec())
            .build()
    }

    pub(crate) fn ecc_p256_params() -> ([u8; 32], [u8; 32]) {
        let params = ecc_sec1_uncompressed();

        assert_eq!(params.len(), 1 + 32 + 32);

        // skip the 0x04 ecc ansi encoding
        let x = params[1..33].try_into().unwrap();
        let y = params[33..].try_into().unwrap();

        (x, y)
    }

    pub(crate) fn ecc_sec1_uncompressed() -> Vec<u8> {
        PUB_ECC_KEY_PARAMS
            .split(":")
            .flat_map(|s| s.split_whitespace())
            .filter(|s| !s.is_empty())
            .map(|s| u8::from_str_radix(s, 16).expect("should be hex"))
            .collect()
    }

    #[test]
    fn init_exchange_roundrip() {
        let case = InitExchange {
            seq: 42,
            alg: Alg::EcdhP256HkdfSha256Aes256Gcm,
            pub_key: cose_key(),
            hkdf_salt: [
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7,
                8, 9, 4, 2,
            ],
        };

        let buf = minicbor::to_vec(&case).unwrap();

        let res: InitExchange = minicbor::decode(&buf).unwrap();

        assert_eq!(res, case);

        with_insta!({
            assert_snapshot!(case);

            assert_snapshot!(Hexdump(buf));
        });
    }

    #[test]
    fn exchange_resp_roundrip() {
        let case = ExchangeResp {
            seq: 42,
            pub_key: cose_key(),
        };

        let buf = minicbor::to_vec(&case).unwrap();

        let res: ExchangeResp = minicbor::decode(&buf).unwrap();

        assert_eq!(res, case);

        with_insta!({
            assert_snapshot!(case);

            assert_snapshot!(Hexdump(buf));
        });
    }
}
