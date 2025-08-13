use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use reqwest::Client;

use crate::downloaders;
use crate::models::{AlbumYear, DownloadConfig, Service, SkipConfig, Track, WorkerIds};

#[expect(
    clippy::too_many_arguments,
    reason = "this function is called from a single place"
)]
pub async fn run_album_worker(
    client: Client,
    urls: Arc<Mutex<Vec<String>>>,
    output_path: PathBuf,
    force_download: bool,
    album_year: Option<AlbumYear>,
    flatten_directories: bool,
    config: DownloadConfig,
    track_workers: usize,
    skip: SkipConfig,
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
            force_download,
            album_year,
            flatten_directories,
            config.clone(),
            track_workers,
            skip,
            running.clone(),
            album_worker,
        )
        .await;
    }

    eprintln!("[WORKER {album_worker}] stopped");
}

#[expect(clippy::type_complexity)]
#[expect(
    clippy::too_many_arguments,
    reason = "this function is called from a single place"
)]
pub async fn run_track_worker(
    client: Client,
    service: Service,
    tracks: Arc<Mutex<Vec<(Option<u32>, Track)>>>,
    track_count: u32,
    token_expiry: u64,
    force_download: bool,
    config: DownloadConfig,
    album_path: Arc<PathBuf>,
    workers: WorkerIds,
) {
    loop {
        let Some((track_number, track)) = tracks.lock().unwrap().pop() else {
            return;
        };

        downloaders::download_track(
            client.clone(),
            service,
            &track,
            track_number,
            track_count,
            token_expiry,
            force_download,
            &config,
            album_path.clone(),
            workers,
        )
        .await;
    }
}
