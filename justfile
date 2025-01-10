set dotenv-load := true

initialize_membership slot:
    cargo run --release -- --slot {{slot}} membership --max-validator-index 100 --out ./membership_proof_{{slot}}.bin initialize

update_membership prior_slot slot:
    cargo run --release -- --slot {{slot}} membership --max-validator-index 101 --out ./membership_proof_{{slot}}.bin update ./membership_proof_{{prior_slot}}.bin

aggregate slot:
    cargo run --release -- --slot {{slot}} aggregate --out ./aggproof_{{slot}}.bin ./proof1.bin
