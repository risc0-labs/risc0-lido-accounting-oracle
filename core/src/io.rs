use alloy_primitives::B256;
use risc0_zkvm::sha::Digest;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Input {
    /// The Program ID of this program. Need to accept it as input rather than hard-code otherwise it creates a cyclic hash reference
    /// This MUST be written to the journal and checked by the verifier! See https://github.com/risc0/risc0-ethereum/blob/main/contracts/src/RiscZeroSetVerifier.sol#L114
    pub self_program_id: Digest,

    /// The withdrawal credentials we are searching for a match with
    pub withdrawal_credentials: B256,
    /// The state root of the state used in the current proof
    pub current_state_root: B256,
    /// the top validator index the membership proof will be extended to
    pub up_to_validator_index: u64,

    /// If this the first proof in the sequence, or a continuation that consumes an existing proof
    pub proof_type: ProofType,

    /// Merkle SSZ proof that the prior state root is a pre-state of the current state
    pub multiproof: crate::Multiproof,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub enum ProofType {
    Initial,
    Continuation {
        prior_state_root: B256,
        prior_slot: u64,
        prior_up_to_validator_index: u64,
        prior_membership: Vec<u64>,
    },
}

#[derive(serde::Serialize)]
pub struct Journal {
    pub self_program_id: Digest,
    pub state_root: B256,
    pub up_to_validator_index: u64,
    pub withdrawal_credentials: B256,
    pub membership: Vec<u64>,
}
