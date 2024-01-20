//! Defines the `clap` command line interface for `lark-vm`.

use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Opts {
    /// The path to the Meadowlark source file to be compiled.
    #[arg(short, long)]
    pub meadowlark_src: Option<PathBuf>,

    /// Path to the ROM's lark source file.
    #[arg(short, long)]
    pub lark_src: Option<PathBuf>,

    /// The path to the ROM file that will be run.
    #[arg(short, long)]
    pub romfile: Option<PathBuf>,

    /// Start in debug mode?
    #[arg(short, long)]
    pub debug: bool,
}

impl Opts {
    pub fn rom_src_path(&self) -> PathBuf {
        self.lark_src.as_ref().map(Clone::clone).unwrap_or_else(|| {
            self.romfile
                .clone()
                .expect("No lark source file provided")
                .with_extension("")
                .with_extension("lark")
        })
    }
}
