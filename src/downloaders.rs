use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures::future;
use reqwest::Client;
use tokio::time;

use crate::models::{PageData, Track};
use crate::{requests, text_utils, workers};

#[expect(
    clippy::too_many_arguments,
    reason = "this function is called from a single place"
)]
pub async fn download_album(
    client: Client,
    url: &str,
    output_path: &Path,
    country: String,
    track_workers: usize,
    skip_tracks: bool,
    skip_cover: bool,
    running: Arc<AtomicBool>,
    album_worker: usize,
) {
    eprintln!("[WORKER {album_worker}] resolving album {url}");

    let html = loop {
        let Some(html) =
            requests::resolve_album(&client, url, &country, &running, album_worker).await
        else {
            return;
        };

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
                return;
            }

            time::sleep(Duration::from_secs(5)).await;
        } else {
            break html;
        }
    };

    let page_data = json5::from_str::<PageData>(text_utils::parse_enclosed_value(
        ",{\"type\":\"data\",\"data\":",
        ",\"uses\":{\"url\":1}}];\n",
        &html,
    ))
    .unwrap();

    let tracks_len = page_data.info.tracks.len();

    eprintln!(
        "[WORKER {album_worker}] downloading album {} - {} with {tracks_len} tracks",
        page_data.info.artists[0].name, page_data.info.title
    );

    let album_path = PathBuf::from_iter([
        output_path,
        Path::new(&text_utils::sanitize_file_name(
            &page_data.info.artists[0].name,
        )),
        Path::new(&text_utils::sanitize_file_name(&page_data.info.title)),
    ]);

    fs::create_dir_all(&album_path).unwrap();

    let tracks = Arc::new(Mutex::new(
        page_data
            .info
            .tracks
            .into_iter()
            .enumerate()
            .map(|(i, track)| (u32::try_from(i).unwrap() + 1, track))
            .rev()
            .collect(),
    ));

    let album_path = Arc::new(album_path);

    if !skip_tracks {
        let workers = track_workers.min(tracks_len);

        eprintln!("[WORKER {album_worker}] spawning {workers} track workers");

        for result in future::join_all((1..=workers).map(|track_worker| {
            tokio::spawn(workers::run_track_worker(
                client.clone(),
                tracks.clone(),
                page_data.info.track_count,
                page_data.token_expiry,
                country.clone(),
                album_path.clone(),
                album_worker,
                track_worker,
            ))
        }))
        .await
        {
            result.unwrap();
        }
    }

    if !skip_cover {
        download_album_cover(
            client,
            &page_data.info.title,
            &page_data.info.cover_artwork[0].url,
            &album_path,
            album_worker,
        )
        .await;
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "this function is called from a single place"
)]
pub async fn download_track(
    client: Client,
    track: &Track,
    track_number: u32,
    track_count: u32,
    token_expiry: u64,
    country: &str,
    album_path: Arc<PathBuf>,
    album_worker: usize,
    track_worker: usize,
) {
    if track.producers.is_none() {
        eprintln!(
            "[WORKER {album_worker}-{track_worker}] skipping unavailable track {}",
            track.title
        );

        return;
    }

    eprintln!(
        "[WORKER {album_worker}-{track_worker}] downloading track {}",
        track.title
    );

    let (download, mime_type) = 'track_download: loop {
        let fetch_stream = requests::request_track_download(
            &client,
            track,
            token_expiry,
            country,
            album_worker,
            track_worker,
        )
        .await;

        let mut last_status: Option<(String, String, Instant)> = None;

        loop {
            let Some(fetch_request) =
                requests::track_download_status(&client, &fetch_stream, album_worker, track_worker)
                    .await
            else {
                continue 'track_download;
            };

            if last_status.as_ref().is_none_or(|last_status| {
                (&fetch_request.status, &fetch_request.message) != (&last_status.0, &last_status.1)
            }) {
                eprintln!(
                    "[WORKER {album_worker}-{track_worker}] new download status: {}: {}",
                    fetch_request.status,
                    fetch_request.message.replace("{item}", &track.title)
                );

                last_status = Some((
                    fetch_request.status.clone(),
                    fetch_request.message,
                    Instant::now(),
                ));
            } else if let Some(last_status) = last_status.as_ref() {
                if last_status.2.elapsed() >= Duration::from_secs(30) {
                    eprintln!(
                        "[WORKER {album_worker}-{track_worker}] download status stuck for 30 seconds on {}: {}, retrying",
                        last_status.0,
                        last_status.1.replace("{item}", &track.title)
                    );

                    continue 'track_download;
                }
            }

            if fetch_request.status == "completed" {
                break;
            }

            time::sleep(Duration::from_secs(1)).await;
        }

        let Some(track) =
            requests::download_track(&client, &fetch_stream, album_worker, track_worker).await
        else {
            continue 'track_download;
        };

        break track;
    };

    let file_extension = match mime_type.as_str() {
        "audio/flac" => "flac",
        _ => panic!("unsupported mime type {mime_type}"),
    };

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )]
    let file_name = format!(
        "{track_number:03$}. {} - {}.{}",
        text_utils::sanitize_file_name(&track.artists[0].name),
        text_utils::sanitize_file_name(&track.title),
        file_extension,
        (track_count as f32).log10().floor() as usize + 1
    );

    let track_path = album_path.join(&file_name);
    let mut file = BufWriter::new(File::create_new(&track_path).unwrap());
    file.write_all(&download).unwrap();
}

pub async fn download_album_cover(
    client: Client,
    title: &str,
    url: &str,
    album_path: &Path,
    album_worker: usize,
) {
    eprintln!("[WORKER {album_worker}] downloading {title} album cover");

    let stripped_url = url.strip_suffix(".jpg").unwrap();
    let end_index = stripped_url.rfind('_').unwrap() + 1;
    let url = format!("{}org.jpg", &url[..end_index]);

    let Some(cover) = requests::download_album_cover(&client, &url, album_worker).await else {
        return;
    };

    File::create_new(album_path.join("cover.jpg"))
        .unwrap()
        .write_all(&cover)
        .unwrap();
}
