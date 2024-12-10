use alloy_primitives::U256;
use risc0_zkvm::sha::Digest;
use ssz_rs::prelude::Node;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Input {
    /// The Program ID of this program. Need to accept it as input rather than hard-code otherwise it creates a cyclic hash reference
    /// This MUST be written to the journal and checked by the verifier! See https://github.com/risc0/risc0-ethereum/blob/main/contracts/src/RiscZeroSetVerifier.sol#L114
    pub self_program_id: Digest,

    /// The withdrawal credentials we are searching for a match with
    pub withdrawal_credentials: Node,
    /// The state root of the state used in the current proof
    pub current_state_root: U256,
    /// the top validator index the membership proof will be extended to
    pub max_validator_index: u64,

    /// The state root of the state used in the previous proof
    pub prior_state_root: U256,
    /// The slot of the state used in the previous proof
    pub prior_slot: u64,
    /// The maximum validator index in the state used in the previous proof
    pub prior_max_validator_index: u64,
    /// The membership for validators [0, prior_max_validator_index] to be extended
    /// Replace this with a bitfield when ready to optimize
    pub prior_membership: Vec<bool>,

    /// Merkle SSZ proof that the prior state root is a pre-state of the current state
    pub multiproof: crate::Multiproof,
}

#[derive(serde::Serialize)]
pub struct Journal {
    pub self_program_id: Digest,
    pub state_root: U256,
    pub max_validator_index: u64,
    pub withdrawal_credentials: Node,
    pub membership: Vec<bool>,
}
