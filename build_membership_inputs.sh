# ./target/release/cli --slot 10841902 build --out ./mainnet-inputs/input_membership_initialization_10841902_0_to_200000.bin initial --max-validator-index 200000

# ./target/release/cli --slot 10841902 build --out ./mainnet-inputs/input_membership_continuation_10841902_200000_to_400000.bin continuation-from 10841902 200000 --max-validator-index 400000

# ./target/release/cli --slot 10841902 build --out ./mainnet-inputs/input_membership_continuation_10841902_400000_to_600000.bin continuation-from 10841902 400000 --max-validator-index 600000 

# ./target/release/cli --slot 10841902 build --out ./mainnet-inputs/input_membership_continuation_10841902_600000_to_800000.bin continuation-from 10841902 600000 --max-validator-index 800000 

# ./target/release/cli --slot 10841902 build --out ./mainnet-inputs/input_membership_continuation_10841902_800000_to_1000000.bin continuation-from 10841902 800000 --max-validator-index 1000000 
 
# ./target/release/cli --slot 10841902 build --out ./mainnet-inputs/input_membership_continuation_10841902_1000000_to_1200000.bin continuation-from 10841902 1000000 --max-validator-index 1200000 
./target/release/cli --slot 10841902 build --out ./mainnet-inputs/input_membership_continuation_10841902_1200000_to_1400000.bin continuation-from 10841902 1200000 --max-validator-index 1400000 


./target/release/cli --slot 10841902 build --out ./mainnet-inputs/input_membership_continuation_10841902_1400000_to_end.bin continuation-from 10841902 1400000
