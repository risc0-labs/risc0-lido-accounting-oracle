set dotenv-load := true

initialize:
    cargo run --release -- --slot 10648063 update --max-validator-index 100
