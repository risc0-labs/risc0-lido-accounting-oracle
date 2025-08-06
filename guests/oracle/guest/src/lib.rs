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

use alloy_primitives::Address;
use alloy_sol_types::SolValue;
use bincode::deserialize;
use lido_oracle_core::oracle::generate_oracle_report;
use lido_oracle_core::InputWithReceipt;
use risc0_steel::ethereum::EthChainSpec;
use risc0_zkvm::guest::env;

pub fn entry(
    spec: &EthChainSpec,
    withdrawal_vault_address: Address,
    membership_program_id: [u32; 8],
) {
    env::log("Reading input");
    let input_bytes = env::read_frame();

    env::log("Deserializing input");
    let InputWithReceipt {
        input,
        receipt: membership_receipt,
    } = deserialize(&input_bytes).expect("Failed to deserialize input");

    let journal = generate_oracle_report(
        spec,
        &input,
        membership_receipt.expect("Membership receipt is required"),
        membership_program_id,
        withdrawal_vault_address,
    )
    .expect("Failed to Generate oracle report");

    env::commit_slice(&journal.abi_encode());
}
