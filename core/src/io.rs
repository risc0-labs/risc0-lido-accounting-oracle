use alloy_primitives::B256;
use bitvec::prelude::*;
use risc0_zkvm::sha::Digest;

pub mod validator_membership {
    use super::*;

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct Input {
        /// The Program ID of this program. Need to accept it as input rather than hard-code otherwise it creates a cyclic hash reference
        /// This MUST be written to the journal and checked by the verifier! See https://github.com/risc0/risc0-ethereum/blob/main/contracts/src/RiscZeroSetVerifier.sol#L114
        pub self_program_id: Digest,

        /// The state root of the state used in the current proof
        pub current_state_root: B256,

        /// the top validator index the membership proof will be extended to
        pub up_to_validator_index: u64,

        /// If this the first proof in the sequence, or a continuation that consumes an existing proof
        pub proof_type: ProofType,

        /// Merkle SSZ proof rooted in the beacon state
        pub multiproof: crate::Multiproof,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    pub enum ProofType {
        Initial,
        Continuation {
            prior_state_root: B256,
            prior_slot: u64,
            prior_up_to_validator_index: u64,
            prior_membership: BitVec<u32, Lsb0>,
        },
    }

    #[derive(serde::Serialize)]
    pub struct Journal {
        pub self_program_id: Digest,
        pub state_root: B256,
        pub up_to_validator_index: u64,
        pub membership: BitVec<u32, Lsb0>,
    }
}

pub mod balance_and_exits {
    use super::*;

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct Input {
        /// Block that the proof is rooted in
        pub block_root: B256,

        /// Bitfield indicating which validators are members of the Lido set
        pub membership: BitVec<u32, Lsb0>,

        /// Merkle SSZ proof rooted in the beacon block
        pub multiproof: crate::Multiproof,
    }

    #[derive(serde::Serialize)]
    pub struct Journal {
        pub block_root: B256,
        pub cl_balance: u64,
        pub num_validators: u64,
        pub num_exited_validators: u64,
    }
}
