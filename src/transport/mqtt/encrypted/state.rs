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

//! Encrypted connection state

use std::num::Wrapping;

use aws_lc_rs::agreement::ECDH_P256;
use coset::{CoseKey, CoseKeyBuilder};
use tracing::{error, info};

use crate::transport::mqtt::encrypted::messages::Alg;

use super::messages::InitExchange;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum EncError {
    #[error("couldn't init the handshake")]
    Init,
    #[error("couldn't derive shared secret")]
    Secret,
}

#[derive(Debug, Default)]
pub(crate) struct EncState {
    seq: Wrapping<u16>,
    salt: [u8; 32],
    priv_key: Option<aws_lc_rs::agreement::PrivateKey>,
    // FIXME: zeroize this secret
    secret: Option<[u8; aws_lc_rs::cipher::AES_256_KEY_LEN]>,
}

impl EncState {
    fn fetch_add(&mut self) -> u16 {
        let seq = self.seq.0;

        self.seq += 1;

        seq
    }

    fn init(&mut self) -> Result<InitExchange, EncError> {
        let seq = self.fetch_add();

        let priv_key = self.priv_key.insert(
            aws_lc_rs::agreement::PrivateKey::generate(&ECDH_P256).map_err(|error| {
                error!(%error, "couldn't generate private key");

                EncError::Init
            })?,
        );

        let pub_key = priv_key.compute_public_key().map_err(|error| {
            error!(%error, "couldn't compute public key");

            EncError::Init
        })?;

        let pub_key = CoseKeyBuilder::new_ec2_pub_key_sec1_octet_string(
            coset::iana::EllipticCurve::P_256,
            pub_key.as_ref(),
        )
        .map_err(|error| {
            error!(%error, "couldn't parse cose public key");

            EncError::Init
        })?
        .build();

        aws_lc_rs::rand::fill(&mut self.salt).map_err(|error| {
            error!(%error, "couldn't create hkdf salt");

            EncError::Init
        })?;

        let init = InitExchange {
            seq,
            alg: Alg::EcdhP256HkdfSha256Aes256Gcm,
            pub_key,
            hkdf_salt: self.salt,
        };

        Ok(init)
    }

    fn secret(&mut self, peer_public_key: CoseKey) -> Result<(), EncError> {
        let peer_public_key = peer_public_key.to_sec1_octet_string().map_err(|error| {
            error!(%error, "couldn't convert pub key to sec1");

            EncError::Secret
        })?;
        let peer_public_key = aws_lc_rs::agreement::UnparsedPublicKey::new(
            &aws_lc_rs::agreement::ECDH_P256,
            peer_public_key,
        );

        let priv_key = self.priv_key.take().ok_or_else(|| {
            error!("missing private key");

            EncError::Secret
        })?;

        info!("start agree");
        // TODO: save secret and zeroize
        let key =
            aws_lc_rs::agreement::agree(&priv_key, peer_public_key, EncError::Secret, |sh_se| {
                info!("start");

                let salt = aws_lc_rs::hkdf::Salt::new(aws_lc_rs::hkdf::HKDF_SHA256, &self.salt);

                let pseudo_random_key = salt.extract(sh_se);

                let out_key = pseudo_random_key
                    .expand(&[b"astarte-kdf"], &aws_lc_rs::aead::AES_256_GCM)
                    .map_err(|_| {
                        error!("couldn't expand key");

                        EncError::Secret
                    })?;

                let mut key = [0u8; aws_lc_rs::cipher::AES_256_KEY_LEN];

                out_key.fill(&mut key).map_err(|_| {
                    error!("couldn't fill key");

                    EncError::Secret
                })?;

                info!("key");

                Ok(key)
            })?;

        self.secret = Some(key);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::transport::mqtt::encrypted::messages::tests::cose_key;

    use super::*;

    #[test]
    fn should_init() {
        let mut state = EncState {
            seq: Wrapping(0),
            salt: [0; 32],
            secret: None,
            priv_key: None,
        };

        let res = state.init().unwrap();

        assert_eq!(res.seq, 0);
        assert_eq!(res.alg, Alg::EcdhP256HkdfSha256Aes256Gcm);
        assert_ne!(res.hkdf_salt, [0; 32]);

        res.pub_key.to_sec1_octet_string().unwrap();

        assert_ne!(state.salt, [0; 32]);
        assert_eq!(state.seq.0, 1);
        assert!(state.priv_key.is_some());
        assert_eq!(state.secret, None);
    }

    #[test]
    fn should_derive_secret() {
        tracing_subscriber::fmt().with_test_writer().init();

        let mut state = EncState {
            seq: Wrapping(0),
            salt: [0; 32],
            secret: None,
            priv_key: None,
        };

        state.init().unwrap();
        state.secret(cose_key()).unwrap();

        assert!(state.priv_key.is_none());
        assert!(state.secret.is_some());
    }
}
