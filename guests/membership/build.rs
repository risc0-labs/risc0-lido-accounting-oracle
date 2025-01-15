// Copyright 2023 RISC Zero, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{collections::HashMap, env};

use risc0_build::{embed_methods_with_options, DockerOptions, GuestOptions};

fn main() {
    // Builds can be made deterministic, and thereby reproducible, by using Docker to build the
    // guest. Check the RISC0_USE_DOCKER variable and use Docker to build the guest if set.
    println!("cargo:rerun-if-env-changed=RISC0_USE_DOCKER");
    let use_docker = env::var("RISC0_USE_DOCKER").ok().map(|_| DockerOptions {
        root_dir: Some("../".into()),
    });

    let guest_features = env::var("CARGO_FEATURE_SEPOLIA")
        .map(|_| vec!["sepolia".into()])
        .unwrap_or_default();

    println!(
        "cargo:warning=building guest with features: {:?}",
        guest_features
    );

    // Generate Rust source files for the methods crate.
    embed_methods_with_options(HashMap::from([(
        "validator_membership",
        GuestOptions {
            features: guest_features,
            use_docker: use_docker.clone(),
        },
    )]));
}
