/// ATTRIBUTION: This library is based on the work of Succinct Labs https://github.com/succinctlabs/lidox/blob/195dc5f126be95d7994052511cde514ce998b3a9/contracts/src/SuccinctLidoOracleV1.sol#L102

pragma solidity ^0.8.20;

library BlockRoots {
    error TimestampOutOfRange();
    error NoBlockRootFound();

    /// @notice The address of the beacon roots precompile (regardless of chain).
    /// @dev https://eips.ethereum.org/EIPS/eip-4788
    address internal constant BEACON_ROOTS = 0x000F3df6D732807Ef1319fB7B8bB8522d0Beac02;

    /// @notice The length of the beacon roots ring buffer.
    uint256 internal constant BEACON_ROOTS_HISTORY_BUFFER_LENGTH = 8191;

    /// @notice Attempts to find the block root for the given slot.
    /// @param _slot The slot to get the block root for.
    /// @return blockRoot The beacon block root of the given slot.
    /// @dev BEACON_ROOTS returns a block root for a given parent block's timestamp. To get the block root for slot
    ///      N, you use the timestamp of slot N+1. If N+1 is not avaliable, you use the timestamp of slot N+2, and
    //       so on.
    function findBlockRoot(uint256 _genesis_block_timestamp, uint256 _slot) internal view returns (bytes32 blockRoot) {
        uint256 currBlockTimestamp = _genesis_block_timestamp + ((_slot + 1) * 12);

        uint256 earliestBlockTimestamp = block.timestamp - (BEACON_ROOTS_HISTORY_BUFFER_LENGTH * 12);
        if (currBlockTimestamp <= earliestBlockTimestamp) {
            revert("Timestamp out of range");
        }

        while (currBlockTimestamp <= block.timestamp) {
            (bool success, bytes memory result) = BEACON_ROOTS.staticcall(abi.encode(currBlockTimestamp));
            if (success && result.length > 0) {
                return abi.decode(result, (bytes32));
            }

            unchecked {
                currBlockTimestamp += 12;
            }
        }

        revert("Block root not found");
    }
}
