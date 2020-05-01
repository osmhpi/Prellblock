//! Run the following command to create a public and private key pair for `name`:
//!
//! `cargo run --bin gen-key <name>`

use hexutil::ToHex;
use pinxit::Identity;
use std::{env::args, fs};

fn main() {
    let identity = Identity::generate();
    let name = args().nth(1).expect("identity name");
    fs::write(format!("config/{0}/{0}.key", name), identity.to_hex()).unwrap();
    fs::write(format!("config/{0}/{0}.pub", name), identity.id().to_hex()).unwrap();
}
