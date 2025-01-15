set dotenv-load := true

build:
    cargo build --release --features sepolia


## Input building tasks

build_input_initialization slot: build
    ./target/release/cli --slot {{slot}} build --out ./input_membership_initialization_{{slot}}.bin initial

build_input_continuation prior_slot slot: build
    ./target/release/cli --slot {{slot}} build --out ./input_membership_continuation_{{prior_slot}}_to_{{slot}}.bin continuation-from {{prior_slot}} 

build_input_aggregation slot: build
    ./target/release/cli --slot {{slot}} build --out ./input_aggregation_{{slot}}.bin aggregation


## Proving tasks

prove_membership_init slot: build
    ./target/release/cli --slot {{slot}} prove --input ./input_membership_initialization_{{slot}}.bin --out ./membership_proof_{{slot}}.bin initial

prove_membership_continuation prior_slot slot: build
    ./target/release/cli --slot {{slot}} prove --input ./input_membership_continuation_{{prior_slot}}_to_{{slot}}.bin --out ./membership_proof_{{slot}}.bin continuation-from ./membership_proof_{{prior_slot}}.bin

prove_aggregate slot: build
    ./target/release/cli --slot {{slot}} prove --input ./input_aggregation_{{slot}}.bin --out ./aggregate_proof_{{slot}}.bin aggregation ./membership_proof_{{slot}}.bin

## helper for doing all the steps

prove_all slot: (build_input_initialization slot) (prove_membership_init slot) (build_input_aggregation slot) (prove_aggregate slot)


## Submission to chain

submit slot: build
    ./target/release/cli --slot {{slot}} submit --proof ./aggregate_proof_{{slot}}.bin

# Deploy contracts

deploy:
    cd contracts && forge script script/Deploy.s.sol --rpc-url $ETH_RPC_URL --broadcast --verify
