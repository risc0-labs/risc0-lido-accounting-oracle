set dotenv-load := true

initialize:
    cargo run --release -- --slot 10648063 membership --max-validator-index 100 --out ./proof1.bin initialize

update:
    cargo run --release -- --slot 10648064 membership --max-validator-index 110 --out ./proof2.bin update ./proof1.bin

