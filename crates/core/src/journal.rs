use alloy_sol_types::sol;
use risc0_steel::Commitment;

sol! {
    struct Journal {
        // LIP-23 oracle fields
        uint256 clBalanceGwei;
        uint256 withdrawalVaultBalanceWei;
        uint256 totalDepositedValidators;
        uint256 totalExitedValidators;
        bytes32 blockRoot;

        // Non-oracle fields commit to Steel environment and membership for continuation
        Commitment commitment;
        bytes32 membershipCommitment;
    }
}
