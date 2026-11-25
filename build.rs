use std::env;

use clap::{CommandFactory, ValueEnum};
use clap_complete::Shell;

use crate::cli::Cli;

#[path = "src/cli.rs"]
mod cli;

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();

    for &shell in Shell::value_variants() {
        clap_complete::generate_to(shell, &mut Cli::command(), "lucida", &out_dir).unwrap();
    }
}
