use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use reqwest::header::CONTENT_TYPE;
use reqwest::{Client, StatusCode, Url};
use tokio::time;

use crate::models::{
    Account, Token, Track, TrackDownload, TrackDownloadRequest, TrackDownloadResult,
    TrackDownloadStatus, Upload,
};

pub async fn resolve_album(
    client: &Client,
    url: &str,
    country: &str,
    running: &Arc<AtomicBool>,
    album_worker: usize,
) -> Option<String> {
    loop {
        let response = client
            .get(
                Url::parse_with_params("https://lucida.to/", &[("url", url), ("country", country)])
                    .unwrap(),
            )
            .send()
            .await
            .unwrap();

        let status = response.status();

        if status == StatusCode::OK {
            break Some(response.text().await.unwrap());
        }

        eprintln!(
            "[WORKER {album_worker}] received code {} when resolving album",
            status.as_u16()
        );

        if !running.load(Ordering::Relaxed) {
            return None;
        }

        time::sleep(Duration::from_secs(5)).await;
    }
}

pub async fn request_track_download(
    client: &Client,
    track: &Track,
    token_expiry: u64,
    country: &str,
    album_worker: usize,
    track_worker: usize,
) -> TrackDownload {
    loop {
        let response = client
            .post("https://lucida.to/api/load?url=%2Fapi%2Ffetch%2Fstream%2Fv2")
            .json(&TrackDownloadRequest {
                account: Account {
                    id: country,
                    r#type: "country",
                },
                compat: false,
                downscale: "original",
                handoff: true,
                metadata: true,
                private: false,
                token: Token {
                    expiry: token_expiry,
                    primary: &track.csrf,
                    secondary: track.csrf_fallback.as_deref(),
                },
                upload: Upload { enabled: false },
                url: &track.url,
            })
            .send()
            .await
            .unwrap();

        let status = response.status();

        if status == StatusCode::OK {
            if let Ok(track_download) = response.json().await {
                match track_download {
                    TrackDownloadResult::Ok(track_download) => break track_download,
                    TrackDownloadResult::Error { error, .. } => {
                        eprintln!(
                            "[WORKER {album_worker}-{track_worker}] error when requesting track download: {error}"
                        );

                        time::sleep(Duration::from_secs(5)).await;
                    }
                }
            } else {
                eprintln!(
                    "[WORKER {album_worker}-{track_worker}] invalid JSON when requesting track download"
                );

                time::sleep(Duration::from_secs(5)).await;
            }
        } else {
            eprintln!(
                "[WORKER {album_worker}-{track_worker}] received code {} when requesting track download",
                status.as_u16()
            );

            time::sleep(Duration::from_secs(5)).await;
        }
    }
}

pub async fn track_download_status(
    client: &Client,
    stream: &TrackDownload,
    album_worker: usize,
    track_worker: usize,
) -> Option<TrackDownloadStatus> {
    loop {
        let response = client
            .get(format!(
                "https://{}.lucida.to/api/fetch/request/{}",
                stream.server, stream.handoff
            ))
            .send()
            .await
            .unwrap();

        let status = response.status();

        if status == StatusCode::OK {
            break Some(response.json().await.unwrap());
        }

        eprintln!(
            "[WORKER {album_worker}-{track_worker}] received code {} when checking track processing status",
            status.as_u16()
        );

        if status == StatusCode::INTERNAL_SERVER_ERROR {
            break None;
        }

        time::sleep(Duration::from_secs(5)).await;
    }
}

pub async fn download_track(
    client: &Client,
    stream: &TrackDownload,
    album_worker: usize,
    track_worker: usize,
) -> Option<(Vec<u8>, String)> {
    loop {
        let response = client
            .get(format!(
                "https://{}.lucida.to/api/fetch/request/{}/download",
                stream.server, stream.handoff
            ))
            .send()
            .await
            .unwrap();

        let status = response.status();

        if status == StatusCode::OK {
            let mime_type = response.headers()[CONTENT_TYPE]
                .to_str()
                .unwrap()
                .to_owned();

            match response.bytes().await {
                Ok(bytes) => break Some((bytes.to_vec(), mime_type)),
                Err(err) => {
                    eprintln!(
                        "[WORKER {album_worker}-{track_worker}] error when downloading track audio: {err}"
                    );
                }
            }
        } else {
            eprintln!(
                "[WORKER {album_worker}-{track_worker}] received code {} when downloading track audio",
                status.as_u16()
            );

            if status == StatusCode::INTERNAL_SERVER_ERROR {
                break None;
            }

            time::sleep(Duration::from_secs(5)).await;
        }
    }
}

pub async fn download_album_cover(
    client: &Client,
    url: &str,
    album_worker: usize,
) -> Option<Vec<u8>> {
    loop {
        let response = client.get(url).send().await.unwrap();

        let status = response.status();

        if status == StatusCode::OK {
            break Some(response.bytes().await.unwrap().to_vec());
        } else if status == StatusCode::NOT_FOUND {
            eprintln!("[WORKER {album_worker}] album doesn't have a cover");
            return None;
        }

        eprintln!(
            "[WORKER {album_worker}] received code {} when downloading album cover from {url}",
            status.as_u16()
        );
    }
}
