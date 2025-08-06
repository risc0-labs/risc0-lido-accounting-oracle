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

mod error;
mod generate_report;
pub mod input;
pub mod journal;

#[cfg(feature = "builder")]
use beacon_state::mainnet::BeaconState;
pub use generate_report::generate_oracle_report;

pub use error::{Error, Result};

use revm::primitives::hardfork::SpecId;
use risc0_steel::config::{ChainSpec, ForkCondition};
use risc0_steel::ethereum::EthChainSpec;
pub use risc0_steel::ethereum::ETH_SEPOLIA_CHAIN_SPEC;
#[cfg(feature = "builder")]
use ssz_multiproofs::{Multiproof, MultiproofBuilder};
use std::collections::BTreeMap;
use std::sync::LazyLock;

type Node = [u8; 32];

pub mod mainnet {
    use alloy_primitives::{address, Address};

    pub const WITHDRAWAL_CREDENTIALS: alloy_primitives::B256 = alloy_primitives::B256::new([
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb9, 0xd7, 0x93,
        0x48, 0x78, 0xb5, 0xfb, 0x96, 0x10, 0xb3, 0xfe, 0x8a, 0x5e, 0x44, 0x1e, 0x8f, 0xad, 0x7e,
        0x29, 0x3f,
    ]);

    pub const WITHDRAWAL_VAULT_ADDRESS: Address =
        address!("b9d7934878b5fb9610b3fe8a5e441e8fad7e293f");
}

pub mod sepolia {
    use alloy_primitives::{address, Address};

    pub const WITHDRAWAL_CREDENTIALS: alloy_primitives::B256 = alloy_primitives::B256::new([
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xde, 0x73, 0x18,
        0xaf, 0xa6, 0x7e, 0xad, 0x6d, 0x6b, 0xbc, 0x82, 0x24, 0xdf, 0xce, 0x5e, 0xd6, 0xe4, 0xb8,
        0x6d, 0x76,
    ]);

    pub const WITHDRAWAL_VAULT_ADDRESS: Address =
        address!("De7318Afa67eaD6d6bbC8224dfCe5ed6e4b86d76");
}

pub static ANVIL_CHAIN_SPEC: LazyLock<EthChainSpec> = LazyLock::new(|| ChainSpec {
    chain_id: 31337,
    forks: BTreeMap::from([(SpecId::PRAGUE, ForkCondition::Timestamp(0))]),
});

#[cfg(feature = "builder")]
pub(crate) fn build_with_versioned_state(
    builder: MultiproofBuilder,
    beacon_state: &BeaconState,
) -> Result<Multiproof<'static>> {
    use beacon_state::BeaconState;

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
