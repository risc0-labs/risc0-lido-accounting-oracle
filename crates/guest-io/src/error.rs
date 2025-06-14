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

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[cfg(feature = "builder")]
    #[error("Error in merklization: {0}")]
    Merklization(#[from] ssz_rs::MerkleizationError),

    #[error("SSZ Multiprove error: {0}")]
    SszMultiproof(#[from] ssz_multiproofs::Error),

    #[error("Failed to convert between integers: {0}")]
    IntegerConversion(#[from] std::num::TryFromIntError),

    #[error("The fork version is not currently supported")]
    UnsupportedFork,

    #[error("Historical batch not provided but it is required for proving states are linked over the number of slots they span")]
    MissingHistoricalBatch,

    #[error("Internal serde failed: {0}")]
    Risc0Serde(#[from] risc0_zkvm::serde::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
