// Copyright 2025 RISC Zero, Inc.
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

use crate::Node;
use risc0_zkvm::Receipt;
#[cfg(feature = "builder")]
use {
    crate::Result,
    beacon_state::mainnet::BeaconState,
    ssz_multiproofs::{Multiproof, MultiproofBuilder},
    ssz_rs::prelude::*,
};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct InputWithReceipt<T> {
    pub input: T,
    pub receipt: Option<Receipt>,
}

#[cfg(feature = "builder")]
pub(crate) fn build_with_versioned_state(
    builder: MultiproofBuilder,
    beacon_state: &BeaconState,
) -> Result<Multiproof<'static>> {
    match beacon_state {
        BeaconState::Phase0(b) => Ok(builder.build(b)?),
        BeaconState::Altair(b) => Ok(builder.build(b)?),
        BeaconState::Bellatrix(b) => Ok(builder.build(b)?),
        BeaconState::Capella(b) => Ok(builder.build(b)?),
        BeaconState::Deneb(b) => Ok(builder.build(b)?),
        BeaconState::Electra(b) => Ok(builder.build(b)?),
    }
}

/// Slice an 8 byte u64 out of a 32 byte chunk
/// pos gives the position (e.g. first 8 bytes, second 8 bytes, etc.)
pub(crate) fn u64_from_b256(node: &Node, pos: usize) -> u64 {
    u64::from_le_bytes(node[pos * 8..(pos + 1) * 8].try_into().unwrap())
}
