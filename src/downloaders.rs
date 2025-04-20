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

use crate::models::{Info, PageData, Service, Track};
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

    let (album_title, cover_artwork, artist_name, tracks, track_count, tracks_len) =
        match page_data.info {
            Info::Album {
                title,
                mut cover_artwork,
                mut artists,
                track_count,
                tracks,
            } => {
                let tracks_len = tracks.len();

                (
                    title,
                    cover_artwork.pop().unwrap(),
                    artists
                        .pop()
                        .map_or_else(|| "Unknown".into(), |artist| artist.name),
                    tracks
                        .into_iter()
                        .enumerate()
                        .map(|(i, track)| (Some(u32::try_from(i).unwrap() + 1), track))
                        .rev()
                        .collect(),
                    track_count,
                    tracks_len,
                )
            }
            Info::Track {
                url,
                title,
                artists,
                mut album,
                producers,
            } => (
                album.title,
                album.cover_artwork.pop().unwrap(),
                album.artists.pop().map_or_else(
                    || artists.last().unwrap().name.clone(),
                    |artist| artist.name,
                ),
                vec![(
                    None,
                    Track {
                        title,
                        url,
                        artists,
                        producers,
                        csrf: page_data.token,
                        csrf_fallback: None,
                    },
                )],
                album.track_count.unwrap_or(1),
                1,
            ),
        };

    eprintln!(
        "[WORKER {album_worker}] downloading album {artist_name} - {album_title} with {tracks_len} tracks"
    );

    let album_path = PathBuf::from_iter([
        output_path,
        Path::new(&text_utils::sanitize_file_name(&artist_name)),
        Path::new(&text_utils::sanitize_file_name(&album_title)),
    ]);

    fs::create_dir_all(&album_path).unwrap();

    let tracks = Arc::new(Mutex::new(tracks));
    let album_path = Arc::new(album_path);

    if !skip_tracks {
        let workers = track_workers.min(tracks_len);

        eprintln!("[WORKER {album_worker}] spawning {workers} track workers");

        for result in future::join_all((1..=workers).map(|track_worker| {
            tokio::spawn(workers::run_track_worker(
                client.clone(),
                page_data.original_service,
                tracks.clone(),
                track_count,
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
            &album_title,
            page_data.original_service,
            &cover_artwork.url,
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
    service: Service,
    track: &Track,
    track_number: Option<u32>,
    track_count: u32,
    token_expiry: u64,
    country: &str,
    album_path: Arc<PathBuf>,
    album_worker: usize,
    track_worker: usize,
) {
    // HACK(jel): this seems to be the only way to detect tracks that are impossible
    // to download yet
    if matches!(service, Service::Qobuz) && track.producers.is_none() {
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
        let track_download = requests::request_track_download(
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
            let Some(track_download) = requests::track_download_status(
                &client,
                &track_download,
                album_worker,
                track_worker,
            )
            .await
            else {
                continue 'track_download;
            };

            if last_status.as_ref().is_none_or(|last_status| {
                (&track_download.status, &track_download.message)
                    != (&last_status.0, &last_status.1)
            }) {
                eprintln!(
                    "[WORKER {album_worker}-{track_worker}] new download status: {}: {}",
                    track_download.status,
                    track_download.message.replace("{item}", &track.title)
                );

                last_status = Some((
                    track_download.status.clone(),
                    track_download.message,
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

            if track_download.status == "completed" {
                break;
            }

            time::sleep(Duration::from_secs(1)).await;
        }

        let Some(track) =
            requests::download_track(&client, &track_download, album_worker, track_worker).await
        else {
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
