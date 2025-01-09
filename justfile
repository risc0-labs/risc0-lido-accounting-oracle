set dotenv-load := true

initialize:
    cargo run --release -- --slot 1000 membership --out ./proof1.bin initialize

# update:
#     cargo run --release -- --slot 4636801 membership --max-validator-index 110 --out ./proof2.bin update ./proof1.bin

aggregate:
    cargo run --release -- --slot 1000 aggregate --out ./aggproof.bin ./proof1.bin
