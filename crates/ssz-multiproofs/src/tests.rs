use crate::*;
use ethereum_consensus::phase0::presets::mainnet::BeaconState;
use risc0_zkvm::serde::{from_slice, to_vec};
use ssz_rs::prelude::*;

fn test_roundtrip_serialization(multiproof: &Multiproof) {
    let serialized = to_vec(multiproof).unwrap();
    let deserialized: Multiproof = from_slice(&serialized).unwrap();
    assert_eq!(multiproof, &deserialized);
}

#[test]
fn test_proving_validator_fields() {
    let beacon_state = BeaconState::default();

    let builder = MultiproofBuilder::new();
    let multiproof = builder
        .with_path::<BeaconState>(&["validators".into()])
        .build(&beacon_state)
        .unwrap();

    multiproof
        .verify(&beacon_state.hash_tree_root().unwrap())
        .unwrap();

    // Add a validator to the state
    let mut beacon_state = BeaconState::default();
    beacon_state.validators.push(Default::default());

    let gindex = BeaconState::generalized_index(&[
        "validators".into(),
        0.into(),
        "withdrawal_credentials".into(),
    ])
    .expect("Invalid path for state_roots");

    let multiproof = MultiproofBuilder::new()
        .with_gindex(gindex)
        .build(&beacon_state)
        .unwrap();

    multiproof
        .verify(&beacon_state.hash_tree_root().unwrap())
        .unwrap();

    assert_eq!(
        multiproof.values().next(),
        Some((
            gindex as u64,
            &super::Node::from_slice(beacon_state.validators[0].withdrawal_credentials.as_slice())
        ))
    );

    test_roundtrip_serialization(&multiproof);
}

#[test]
fn test_proving_state_roots() {
    let beacon_state = BeaconState::default();

    let gindex = BeaconState::generalized_index(&["state_roots".into(), 10.into()])
        .expect("Invalid path for state_roots");

    let multiproof = MultiproofBuilder::new()
        .with_gindex(gindex)
        .build(&beacon_state)
        .unwrap();

    multiproof
        .verify(&beacon_state.hash_tree_root().unwrap())
        .unwrap();

    assert_eq!(
        multiproof.values().next(),
        Some((gindex as u64, &beacon_state.state_roots[10]))
    );

    test_roundtrip_serialization(&multiproof);
}
