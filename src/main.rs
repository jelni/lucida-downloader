use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use clap::Parser;
use futures::future;
use models::Cli;
use reqwest::Client;
use tokio::signal;

mod downloaders;
mod models;
mod requests;
mod text_utils;
mod workers;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli = Cli::parse();

    let mut urls = cli.urls;

    for file in cli.file {
        urls.extend(
            BufReader::new(File::open(file).unwrap())
                .lines()
                .map(|line| line.unwrap()),
        );
    }

    urls.reverse();

    if urls.is_empty() {
        eprintln!("no URLs to download");
        return;
    }

    let urls_len = urls.len();

    eprintln!("downloading {urls_len} albums");

    let client = Client::new();
    let urls = Arc::new(Mutex::new(urls));
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();
    let workers = cli.album_workers.min(urls_len);

    eprintln!("spawning {workers} album workers");

    tokio::spawn(async move {
        signal::ctrl_c().await.unwrap();
        running_clone.store(false, Ordering::Relaxed);
        eprintln!("Stopping gracefully");
    });

    let output = cli.output.unwrap_or_else(|| env::current_dir().unwrap());

    for result in future::join_all((1..=workers).map(|album_worker| {
        tokio::spawn(workers::run_album_worker(
            client.clone(),
            urls.clone(),
            output.clone(),
            cli.country.clone(),
            cli.track_workers,
            cli.skip_tracks,
            cli.skip_cover,
            running.clone(),
            album_worker,
        ))
    }))
    .await
    {
        result.unwrap();
    }

    eprintln!("finished!");
}
