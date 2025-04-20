use std::path::PathBuf;

use clap::Parser;
use serde::{Deserialize, Serialize};

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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageData {
    pub info: Info,
    pub original_service: Service,
    pub token: String,
    pub token_expiry: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "type")]
pub enum Info {
    #[serde(rename_all = "camelCase")]
    Album {
        title: String,
        cover_artwork: Vec<CoverArtwork>,
        artists: Vec<Artist>,
        track_count: u32,
        tracks: Vec<Track>,
    },
    Track {
        url: String,
        title: String,
        artists: Vec<Artist>,
        album: Album,
        producers: Option<Vec<String>>,
    },
}

#[derive(Deserialize)]
pub struct CoverArtwork {
    pub url: String,
}

#[derive(Deserialize)]
pub struct Artist {
    pub name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub title: String,
    pub url: String,
    pub artists: Vec<Artist>,
    pub producers: Option<Vec<String>>,
    pub csrf: String,
    pub csrf_fallback: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub title: String,
    pub cover_artwork: Vec<CoverArtwork>,
    pub artists: Vec<Artist>,
    pub track_count: Option<u32>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Service {
    Qobuz,
    Tidal,
}

#[expect(clippy::struct_excessive_bools)]
#[derive(Serialize)]
pub struct TrackDownloadRequest<'a> {
    pub account: Account<'a>,
    pub compat: bool,
    pub downscale: &'static str,
    pub handoff: bool,
    pub metadata: bool,
    pub private: bool,
    pub token: Token<'a>,
    pub upload: Upload,
    pub url: &'a str,
}

#[derive(Serialize)]
pub struct Account<'a> {
    pub id: &'a str,
    pub r#type: &'static str,
}

#[derive(Serialize)]
pub struct Token<'a> {
    pub expiry: u64,
    pub primary: &'a str,
    pub secondary: Option<&'a str>,
}

#[derive(Serialize)]
pub struct Upload {
    pub enabled: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum TrackDownloadResult {
    Ok(TrackDownload),
    Error { error: String },
}

#[derive(Deserialize)]
pub struct TrackDownload {
    pub handoff: String,
    pub server: String,
}

#[derive(Debug, Deserialize)]
pub struct TrackDownloadStatus {
    pub status: String,
    pub message: String,
}
