use std::{
    net::SocketAddrV4,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,
}
#[derive(Subcommand)]
pub enum Commands {
    /// Decode bencoded input string.
    Decode { input: String },
    /// Print info about a torrent file.
    Info { torrent: PathBuf },
    /// Print peer info from the tracker stored in a torrent file.
    Peers { torrent: PathBuf },
    /// Perform a peer handshake and print its peer id.
    Handshake {
        /// Path to the torrent file.
        torrent: PathBuf,
        /// Socket address (IPv4) of the peer to connect to.
        address: SocketAddrV4,
    },
    /// Downloads a single piece from the torrent file.
    #[command(name = "download_piece")]
    DownloadPiece {
        /// Path to store the downloaded file.
        #[arg(short)]
        output_file: PathBuf,
        /// Path to the torrent file.
        torrent: PathBuf,
        /// Piece index to download.
        index: usize,
    },
}
