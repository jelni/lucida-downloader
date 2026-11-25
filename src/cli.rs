use std::path::PathBuf;

use clap::{Parser, ValueEnum};

#[expect(clippy::struct_excessive_bools)]
#[derive(Parser)]
#[command(arg_required_else_help = true)]
pub struct Cli {
    /// URLs to download
    pub urls: Vec<String>,

    /// files to read URLs from
    #[arg(short, long)]
    pub file: Vec<PathBuf>,

    /// custom path to download to
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// overwrite already downloaded files
    #[arg(long)]
    pub force: bool,

    /// place all artist's singles in a "Singles" directory. their covers will
    /// not be downloaded
    #[arg(long)]
    pub group_singles: bool,

    /// use "<album> (year)" or "(year) <album>" directory name
    #[arg(value_enum, long)]
    pub album_year: Option<AlbumYear>,

    /// use "<artist> - <album>" format instead of nested "<artist>/<album>"
    /// directories
    #[arg(long)]
    pub flatten_directories: bool,

    /// country to use accounts from
    #[arg(long, default_value_t = String::from("auto"))]
    pub country: String,

    /// disable metadata embedding by lucida
    #[arg(long)]
    pub no_metadata: bool,

    /// hide tracks from recent downloads on lucida
    #[arg(long)]
    pub private: bool,

    /// amount of albums to download simultaneously
    #[arg(long, default_value_t = 1)]
    pub album_workers: usize,

    /// amount of tracks to download simultaneously for each album
    #[arg(long, default_value_t = 4)]
    pub track_workers: usize,

    /// skip downloading tracks in the album
    #[arg(long)]
    pub skip_tracks: bool,

    /// skip downloading album cover
    #[arg(long)]
    pub skip_cover: bool,

    /// set the `cf_clearance` cookie and the User-Agent header if Cloudflare is
    /// blocking your requests
    #[arg(long)]
    pub cf_clearance: Option<String>,

    /// the User-Agent header to use
    #[arg(long)]
    pub user_agent: Option<String>,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum AlbumYear {
    Append,
    Prepend,
}
