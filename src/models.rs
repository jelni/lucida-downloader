use std::fmt;
use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

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
}

#[derive(Clone, Copy, ValueEnum)]
pub enum AlbumYear {
    Append,
    Prepend,
}

#[derive(Clone)]
pub struct DownloadConfig {
    pub country: String,
    pub metadata: bool,
    pub private: bool,
}

#[derive(Clone, Copy)]
pub struct SkipConfig {
    pub tracks: bool,
    pub cover: bool,
}

pub struct AlbumInfo {
    pub title: String,
    pub release_year: u16,
    pub cover_artwork_url: String,
    pub artist_name: String,
    pub tracks: Vec<(Option<u32>, Track)>,
    pub track_count: u32,
}

impl AlbumInfo {
    pub fn new(info: Info, token: String) -> Self {
        match info {
            Info::Album {
                title,
                mut cover_artwork,
                mut artists,
                track_count,
                release_date,
                tracks,
            } => Self {
                title,
                release_year: release_date.year().try_into().unwrap(),
                cover_artwork_url: cover_artwork.pop().unwrap().url,
                artist_name: artists
                    .pop()
                    .map_or_else(|| "Unknown".into(), |artist| artist.name),
                tracks: tracks
                    .into_iter()
                    .enumerate()
                    .map(|(i, track)| (Some(u32::try_from(i).unwrap() + 1), track))
                    .rev()
                    .collect(),
                track_count,
            },
            Info::Track {
                url,
                title,
                artists,
                mut album,
                producers,
            } => Self {
                title: album.title,
                release_year: album.release_date.year().try_into().unwrap(),
                cover_artwork_url: album.cover_artwork.pop().unwrap().url,
                artist_name: album.artists.pop().map_or_else(
                    || artists.last().unwrap().name.clone(),
                    |artist| artist.name,
                ),
                tracks: vec![(
                    None,
                    Track {
                        title,
                        url,
                        artists,
                        producers,
                        csrf: token,
                        csrf_fallback: None,
                    },
                )],
                track_count: album.track_count.unwrap_or(1),
            },
        }
    }
}

#[derive(Clone, Copy)]
pub struct WorkerIds {
    pub track: usize,
    pub album: usize,
}

impl fmt::Display for WorkerIds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[WORKER {}-{}]", self.album, self.track)
    }
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
        #[serde(with = "time::serde::rfc3339")]
        release_date: OffsetDateTime,
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
    #[serde(with = "time::serde::rfc3339")]
    pub release_date: OffsetDateTime,
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
