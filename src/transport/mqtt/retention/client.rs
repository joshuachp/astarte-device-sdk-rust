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

use std::collections::VecDeque;

use crate::transport::mqtt::client::AsyncClient;

struct ClientWrap<T> {
    client: AsyncClient,
    store: T,
}

/// Keeps track of the outgoing packets
struct RetentionQueue {
    current: usize,
    /// List of the packets that will be sent before a retained packet is sent.
    queue_pkt_ctr: VecDeque<usize>,
}

struct ToManyPacketsError;

impl RetentionQueue {
    fn publish(&mut self) -> Result<(), ToManyPacketsError> {
        // Increase the counter
        self.current = self.current.checked_add(1).ok_or(ToManyPacketsError)?;

        // This is the retention case
        self.queue_pkt_ctr.push_back(self.current);
        self.current = 0;

        // TODO store publish

        Ok(())
    }

    fn outgoing_publish(&mut self) -> Result<(), ToManyPacketsError> {
        self.queue_pkt_ctr.push_back(self.current);
        self.current = 0;

        // TODO save send

        Ok(())
    }
}
