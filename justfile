set dotenv-load := true

build:
    cargo build --release


## Input building tasks

build_input_initialization slot: build
    ./target/release/cli --slot {{slot}} build --out ./input_membership_initialization_{{slot}}.input initial

build_input_continuation prior_slot slot: build
    ./target/release/cli --slot {{slot}} build --out ./input_membership_continuation_{{prior_slot}}_to_{{slot}}.input continuation-from {{prior_slot}} 

build_input_aggregation slot: build
    ./target/release/cli --slot {{slot}} build --out ./input_aggregation_{{slot}}.input aggregation


## Proving tasks

prove_membership_init slot: build
    ./target/release/cli --slot {{slot}} prove --input ./input_membership_initialization_{{slot}}.input --out ./membership_proof_{{slot}}.input initial

prove_membership_continuation prior_slot slot: build
    ./target/release/cli --slot {{slot}} prove --input ./input_membership_continuation_{{prior_slot}}_to_{{slot}}.input --out ./membership_proof_{{slot}}.input continuation-from ./membership_proof_{{prior_slot}}.input

prove_aggregate slot: build
    ./target/release/cli --slot {{slot}} prove --input ./input_aggregation_{{slot}}.input --out ./aggregate_proof_{{slot}}.input aggregation ./membership_proof_{{slot}}.input

## helper for doing all the steps

prove_all slot: (build_input_initialization slot) (prove_membership_init slot) (build_input_aggregation slot) (prove_aggregate slot)


## Submission to chain

submit slot: build
    ./target/release/cli --slot {{slot}} submit --proof ./aggregate_proof_{{slot}}.input

# Deploy contracts

deploy:
    cd contracts && forge script script/Deploy.s.sol --rpc-url $ETH_RPC_URL --broadcast --verify


# Running Tests

test:
    RISC0_DEV_MODE=1 cargo test --features skip-verify
