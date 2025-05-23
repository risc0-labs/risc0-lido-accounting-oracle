set dotenv-load := true

build:
    #!/usr/bin/env bash
    if [[ "${ETH_NETWORK}" == "sepolia" ]]; then
        echo "Building for Sepolia network"
        cargo build --release --features sepolia
    else
        echo "Building for main network"
        cargo build --release
    fi

## Proving tasks

prove_membership_init slot: build
    ./target/release/cli --slot {{slot}} prove --out ./membership_proof_{{slot}}.proof initial

prove_membership_continuation prior_slot slot: build
    ./target/release/cli --slot {{slot}} prove --out ./membership_proof_{{slot}}.proof continuation-from ./membership_proof_{{prior_slot}}.proof

prove_aggregate slot: build
    ./target/release/cli --slot {{slot}} prove --out ./aggregate_proof_{{slot}}.proof aggregation ./membership_proof_{{slot}}.proof

## helper for doing all the steps

prove_all slot: (prove_membership_init slot) (prove_aggregate slot)

## Submission to chain

submit slot: build
    ./target/release/cli --slot {{slot}} submit --proof ./aggregate_proof_{{slot}}.input

# Deploy contracts

deploy:
    cd contracts && forge script script/Deploy.s.sol --rpc-url $ETH_RPC_URL --broadcast --verify


# Running Tests

test:
    RISC0_DEV_MODE=1 cargo test --features skip-verify
