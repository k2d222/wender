use clap::Parser;
use std::{fs, path::PathBuf};

#[derive(Parser, Debug)]
#[command(version = "0.1", author = "Mathis Brossier", about = "")]
struct Cli {
    input: PathBuf,
}

fn main() {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_wesl::language()).unwrap();

    let cli = Cli::parse();

    let source = fs::read_to_string(&cli.input).expect("could not open input file");
    let tree = parser.parse(&source, None).expect("parse failure");
    println!("{tree:?}")
}
