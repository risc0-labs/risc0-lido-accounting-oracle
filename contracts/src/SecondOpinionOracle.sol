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
//
// SPDX-License-Identifier: Apache-2.0

pragma solidity ^0.8.20;

import {IRiscZeroVerifier} from "risc0/IRiscZeroVerifier.sol";
import {ISecondOpinionOracle} from "./ISecondOpinionOracle.sol";
import {BlockRoots} from "./BlockRoots.sol";
import {ImageID} from "./ImageID.sol"; // auto-generated contract after running `cargo build`.

/// @title A starter application using RISC Zero.
/// @notice This basic application holds a number, guaranteed to be even.
/// @dev This contract demonstrates one pattern for offloading the computation of an expensive
///      or difficult to implement function to a RISC Zero guest running on the zkVM.
contract SecondOpinionOracle is ISecondOpinionOracle {

    struct Report {
        uint256 clBalanceGwei;
        uint256 withdrawalVaultBalanceWei;
        uint256 totalDepositedValidators;
        uint256 totalExitedValidators;
    }

    /// @notice RISC Zero verifier contract address.
    IRiscZeroVerifier public immutable verifier;

    /// @notice Image ID of the only zkVM guest to accept verification from.
    bytes32 public constant imageId = ImageID.BALANCE_AND_EXITS_ID;

    /// @notice Oracle reports stored by refSlot.
    mapping (uint256 => Report) public reports;

    /// @notice Initialize the contract, binding it to a specified RISC Zero verifier.
    constructor(IRiscZeroVerifier _verifier) {
        verifier = _verifier;
    }

    /// @notice Set the even number stored on the contract. Requires a RISC Zero proof that the number is even.
    function update(uint256 refSlot, Report calldata r, bytes calldata seal) public {
        // retrieve the beacon block root for the given refslot
        bytes32 blockRoot = BlockRoots.findBlockRoot(refSlot);
        
        // Construct the expected journal data. Verify will fail if journal does not match.
        bytes memory journal = abi.encode(0); // TODO: Encode the actual journal data for the report
        verifier.verify(seal, imageId, sha256(journal));

        // report is now considered valid for the given slot and can be stored
        reports[block.number] = r;
    }

    /// @notice Returns the number stored.
    function getReport(uint256 refSlot) external view returns (
        bool success,
        uint256 clBalanceGwei,
        uint256 withdrawalVaultBalanceWei,
        uint256 totalDepositedValidators,
        uint256 totalExitedValidators
    ) {
        Report memory report = reports[refSlot];
        return (report.clBalanceGwei != 0, report.clBalanceGwei, report.withdrawalVaultBalanceWei, report.totalDepositedValidators, report.totalExitedValidators);
    }
}
