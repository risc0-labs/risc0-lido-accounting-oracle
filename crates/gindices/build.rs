use ssz_rs::GeneralizedIndexable;
use ssz_rs::Path;
use ssz_rs::PathElement;
use std::env;
use std::fs;
use std::io::Write;

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();

    let dest_filepath = std::path::Path::new(&out_dir).join("gen_pre_electra.rs");
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&dest_filepath)
        .unwrap();
    write_gindex_fns::<_, ethereum_consensus::capella::presets::mainnet::BeaconState>(&mut f);

    let dest_filepath = std::path::Path::new(&out_dir).join("gen_post_electra.rs");
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&dest_filepath)
        .unwrap();
    write_gindex_fns::<_, ethereum_consensus::electra::presets::mainnet::BeaconState>(&mut f);

    println!("cargo::rerun-if-changed=build.rs");
}

fn write_gindex_fns<W, G>(w: &mut W)
where
    W: Write,
    G: GeneralizedIndexable,
{
    // Static paths for the BeaconState
    for (name, path) in [
        ("slot", Path::from(&["slot".into()])),
        (
            "validator_count",
            Path::from(&["validators".into(), PathElement::Length]),
        ),
        (
            "state_roots_base",
            Path::from(&["state_roots".into(), 0.into()]),
        ),
        (
            "historical_summaries_base",
            Path::from(&["historical_summaries".into(), 0.into()]),
        ),
        (
            "validator_balance_base",
            Path::from(&["balances".into(), 0.into()]),
        ),
        (
            "validator_withdrawal_credentials_base",
            Path::from(&[
                "validators".into(),
                0.into(),
                "withdrawal_credentials".into(),
            ]),
        ),
        (
            "validator_exit_epoch_base",
            Path::from(&["validators".into(), 0.into(), "exit_epoch".into()]),
        ),
    ] {
        let gindex = G::generalized_index(path).unwrap() as u64;

        w.write_fmt(format_args!(
            "pub fn {:}() -> u64 {{ {:} }}\n",
            name, gindex
        ))
        .unwrap();
    }
}
