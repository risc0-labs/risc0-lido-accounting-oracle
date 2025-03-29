// Copyright 2024 RISC Zero, Inc.
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
mod io;

use alloy_primitives::{address, Address};
use revm::primitives::SpecId;
use risc0_steel::config::ChainSpec;
use std::sync::LazyLock;

#[cfg(not(feature = "sepolia"))] // mainnet
pub const WITHDRAWAL_CREDENTIALS: alloy_primitives::B256 = alloy_primitives::B256::new([
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb9, 0xd7, 0x93, 0x48,
    0x78, 0xb5, 0xfb, 0x96, 0x10, 0xb3, 0xfe, 0x8a, 0x5e, 0x44, 0x1e, 0x8f, 0xad, 0x7e, 0x29, 0x3f,
]);

#[cfg(not(feature = "sepolia"))] // mainnet
pub const WITHDRAWAL_VAULT_ADDRESS: Address = address!("b9d7934878b5fb9610b3fe8a5e441e8fad7e293f");

#[cfg(feature = "sepolia")]
pub const WITHDRAWAL_CREDENTIALS: alloy_primitives::B256 = alloy_primitives::B256::new([
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xde, 0x73, 0x18, 0xaf,
    0xa6, 0x7e, 0xad, 0x6d, 0x6b, 0xbc, 0x82, 0x24, 0xdf, 0xce, 0x5e, 0xd6, 0xe4, 0xb8, 0x6d, 0x76,
]);

#[cfg(feature = "sepolia")]
pub const WITHDRAWAL_VAULT_ADDRESS: Address = address!("De7318Afa67eaD6d6bbC8224dfCe5ed6e4b86d76");

pub static ANVIL_CHAIN_SPEC: LazyLock<ChainSpec> =
    LazyLock::new(|| ChainSpec::new_single(31337, SpecId::CANCUN));

pub static SEPOLIA_CHAIN_SPEC: LazyLock<ChainSpec> =
    LazyLock::new(|| ChainSpec::new_single(11155111, SpecId::LATEST));

pub use error::{Error, Result};
pub use io::*;
