use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use reqwest::Client;

use crate::downloaders;
use crate::models::Track;

#[expect(
    clippy::too_many_arguments,
    reason = "this function is called from a single place"
)]
pub async fn run_album_worker(
    client: Client,
    urls: Arc<Mutex<Vec<String>>>,
    output_path: PathBuf,
    country: String,
    track_workers: usize,
    skip_tracks: bool,
    skip_cover: bool,
    running: Arc<AtomicBool>,
    album_worker: usize,
) {
    while running.load(Ordering::Relaxed) {
        let Some(url) = urls.lock().unwrap().pop() else {
            eprintln!("[WORKER {album_worker}] stopped: no queued albums");
            return;
        };

        downloaders::download_album(
            client.clone(),
            &url,
            &output_path,
            country.clone(),
            track_workers,
            skip_tracks,
            skip_cover,
            running.clone(),
            album_worker,
        )
        .await;
    }

    eprintln!("[WORKER {album_worker}] stopped");
}

#[expect(
    clippy::too_many_arguments,
    reason = "this function is called from a single place"
)]
pub async fn run_track_worker(
    client: Client,
    tracks: Arc<Mutex<Vec<(u32, Track)>>>,
    track_count: u32,
    token_expiry: u64,
    country: String,
    album_path: Arc<PathBuf>,
    album_worker: usize,
    track_worker: usize,
) {
    loop {
        let Some((track_number, track)) = tracks.lock().unwrap().pop() else {
            return;
        };

        downloaders::download_track(
            client.clone(),
            &track,
            track_number,
            track_count,
            token_expiry,
            &country,
            album_path.clone(),
            album_worker,
            track_worker,
        )
        .await;
    }
}
