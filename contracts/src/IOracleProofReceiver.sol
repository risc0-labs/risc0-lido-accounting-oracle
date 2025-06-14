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
//
// SPDX-License-Identifier: Apache-2.0

pragma solidity ^0.8.20;

import {Steel} from "risc0/steel/Steel.sol";

struct Report {
    uint256 clBalanceGwei;
    uint256 withdrawalVaultBalanceWei;
    uint256 totalDepositedValidators;
    uint256 totalExitedValidators;
}

/// @title Receiver of oracle reports and proof data
interface IOracleProofReceiver {
    function update(uint256 refSlot, Report calldata r, bytes calldata seal, Steel.Commitment calldata commitment)
        external;
}
