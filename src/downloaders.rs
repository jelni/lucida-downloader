use std::borrow::Cow;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures::future;
use reqwest::Client;
use tokio::time;

use crate::models::{
    AlbumInfo, AlbumYear, DownloadConfig, PageData, Service, SkipConfig, Track, WorkerIds,
};
use crate::{requests, text_utils, workers};

#[expect(
    clippy::too_many_arguments,
    reason = "this function is called from a single place"
)]
pub async fn download_album(
    client: Client,
    url: &str,
    output_path: &Path,
    album_year: Option<AlbumYear>,
    flatten_directories: bool,
    config: DownloadConfig,
    track_workers: usize,
    skip: SkipConfig,
    running: Arc<AtomicBool>,
    album_worker: usize,
) {
    let Some(page_data) = resolve_album(&client, url, &config, &running, album_worker).await else {
        return;
    };

    let album = AlbumInfo::new(page_data.info, page_data.token);

    eprintln!(
        "[WORKER {album_worker}] downloading album {} - {} with {} tracks",
        album.artist_name, album.title, album.track_count
    );

    let album_path = {
        let sanitized_artist_name = text_utils::sanitize_file_name(&album.artist_name);
        let sanitized_album_title = text_utils::sanitize_file_name(&album.title);

        let album_directory = match album_year {
            Some(AlbumYear::Append) => {
                format!("{} ({})", sanitized_album_title, album.release_year)
            }
            Some(AlbumYear::Prepend) => {
                format!("({}) {}", album.release_year, sanitized_album_title)
            }
            None => sanitized_album_title,
        };

        let album_directory = if flatten_directories {
            vec![format!("{sanitized_artist_name} - {album_directory}")]
        } else {
            vec![sanitized_artist_name, album_directory]
        };

        let mut album_path = PathBuf::from(output_path);
        album_path.extend(album_directory);

        album_path
    };

    fs::create_dir_all(&album_path).unwrap();

    let tracks_len = album.tracks.len();
    let tracks = Arc::new(Mutex::new(album.tracks));
    let album_path = Arc::new(album_path);

    if !skip.tracks {
        let worker_count = track_workers.min(tracks_len);

        eprintln!("[WORKER {album_worker}] spawning {worker_count} track workers");

        for result in future::join_all((1..=worker_count).map(|track_worker| {
            tokio::spawn(workers::run_track_worker(
                client.clone(),
                page_data.original_service,
                tracks.clone(),
                album.track_count,
                page_data.token_expiry,
                config.clone(),
                album_path.clone(),
                WorkerIds {
                    track: track_worker,
                    album: album_worker,
                },
            ))
        }))
        .await
        {
            result.unwrap();
        }
    }

    if !skip.cover {
        download_album_cover(
            client,
            &album.title,
            page_data.original_service,
            &album.cover_artwork_url,
            &album_path,
            album_worker,
        )
        .await;
    }
}

async fn resolve_album(
    client: &Client,
    url: &str,
    config: &DownloadConfig,
    running: &Arc<AtomicBool>,
    album_worker: usize,
) -> Option<PageData> {
    eprintln!("[WORKER {album_worker}] resolving album {url}");

    let html = loop {
        let html =
            requests::resolve_album(client, url, &config.country, running, album_worker).await?;

        if let Some(error) = [
            "An error occured trying to process your request.",
            "Message: \"Cannot contact any valid server\"",
            "An error occurred. Had an issue getting that item, try again.",
        ]
        .into_iter()
        .find(|&error| html.contains(error))
        {
            eprintln!("[WORKER {album_worker}] HTML contains error: {error}");

            if !running.load(Ordering::Relaxed) {
                return None;
            }

            time::sleep(Duration::from_secs(5)).await;
        } else {
            break html;
        }
    };

    Some(
        json5::from_str(text_utils::parse_enclosed_value(
            ",{\"type\":\"data\",\"data\":",
            ",\"uses\":{\"url\":1}}];\n",
            &html,
        ))
        .unwrap(),
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "this function is called from a single place"
)]
pub async fn download_track(
    client: Client,
    service: Service,
    track: &Track,
    track_number: Option<u32>,
    track_count: u32,
    token_expiry: u64,
    config: &DownloadConfig,
    album_path: Arc<PathBuf>,
    workers: WorkerIds,
) {
    // HACK(jel): this seems to be the only way to detect tracks that are impossible
    // to download yet
    if matches!(service, Service::Qobuz) && track.producers.is_none() {
        eprintln!("{workers} skipping unavailable track {}", track.title);

        return;
    }

    eprintln!("{workers} downloading track {}", track.title);

    let (download, mime_type) = 'track_download: loop {
        let track_download =
            requests::request_track_download(&client, track, token_expiry, config, workers).await;

        let mut last_status: Option<(String, String, Instant)> = None;

        loop {
            let Some(track_download) =
                requests::track_download_status(&client, &track_download, workers).await
            else {
                continue 'track_download;
            };

            if last_status.as_ref().is_none_or(|last_status| {
                (&track_download.status, &track_download.message)
                    != (&last_status.0, &last_status.1)
            }) {
                eprintln!(
                    "{workers} new download status: {}: {}",
                    track_download.status,
                    track_download.message.replace("{item}", &track.title)
                );

                last_status = Some((
                    track_download.status.clone(),
                    track_download.message,
                    Instant::now(),
                ));
            } else if let Some(last_status) = last_status.as_ref()
                && last_status.2.elapsed() >= Duration::from_secs(30)
            {
                eprintln!(
                    "{workers} download status stuck for 30 seconds on {}: {}, retrying",
                    last_status.0,
                    last_status.1.replace("{item}", &track.title)
                );

                continue 'track_download;
            }

            if track_download.status == "completed" {
                break;
            }

            time::sleep(Duration::from_secs(1)).await;
        }

        let Some(track) = requests::download_track(&client, &track_download, workers).await else {
            continue 'track_download;
        };

        break track;
    };

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )]
    let track_number = track_number.map_or_else(String::new, |track_number| {
        format!(
            "{track_number:00$}. ",
            (track_count as f32).log10().floor() as usize + 1
        )
    });

    let artist = if let [artist, ..] = track.artists.as_slice() {
        format!("{} - ", text_utils::sanitize_file_name(&artist.name))
    } else {
        String::new()
    };

    let file_extension = match mime_type.as_str() {
        "audio/flac" => "flac",
        _ => panic!("unsupported mime type {mime_type}"),
    };

    let file_name = format!(
        "{track_number}{artist}{}.{}",
        text_utils::sanitize_file_name(&track.title),
        file_extension
    );

    let track_path = album_path.join(&file_name);
    let mut file = BufWriter::new(File::create_new(&track_path).unwrap());
    file.write_all(&download).unwrap();
}

pub async fn download_album_cover(
    client: Client,
    title: &str,
    service: Service,
    url: &str,
    album_path: &Path,
    album_worker: usize,
) {
    eprintln!("[WORKER {album_worker}] downloading {title} album cover");

    let url = match service {
        Service::Qobuz => {
            let stripped_url = url.strip_suffix(".jpg").unwrap();
            let end_index = stripped_url.rfind('_').unwrap() + 1;
            Cow::Owned(format!("{}org.jpg", &url[..end_index]))
        }
        Service::Tidal => Cow::Borrowed(url),
    };

    let Some(cover) = requests::download_album_cover(&client, &url, album_worker).await else {
        return;
    };

    File::create_new(album_path.join("cover.jpg"))
        .unwrap()
        .write_all(&cover)
        .unwrap();
}
