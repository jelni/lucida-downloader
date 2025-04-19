use std::path::PathBuf;

use clap::Parser;
use serde::Deserialize;

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

    /// country to use accounts from
    #[arg(long, default_value_t = String::from("auto"))]
    pub country: String,

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
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageData {
    pub info: Info,
    pub token_expiry: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub title: String,
    pub cover_artwork: Vec<CoverArtwork>,
    pub artists: Vec<Artist>,
    pub track_count: u32,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Deserialize)]
pub struct CoverArtwork {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct Artist {
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub title: String,
    pub url: String,
    pub artists: Vec<Artist>,
    pub producers: Option<Vec<String>>,
    pub csrf: String,
    pub csrf_fallback: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum TrackDownloadResult {
    Ok(TrackDownload),
    Error { error: String },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackDownload {
    pub handoff: String,
    pub server: String,
}

#[derive(Debug, Deserialize)]
pub struct TrackDownloadStatus {
    pub status: String,
    pub message: String,
}
