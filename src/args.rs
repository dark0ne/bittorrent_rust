use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,
}
#[derive(Subcommand)]
pub enum Commands {
    Download {
        /// Path to store the downloaded file.
        #[arg(short)]
        output_file: PathBuf,
        /// Path to the torrent file.
        torrent: PathBuf,
    },
}
