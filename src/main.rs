use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::{env, process};

use clap::Parser;
use futures::future;
use models::{BASE_URL, Cli, DownloadConfig, SkipConfig};
use reqwest::ClientBuilder;
use reqwest::header::{COOKIE, HeaderMap};
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::signal;

use crate::models::Availability;

mod downloaders;
mod models;
mod requests;
mod text_utils;
mod workers;

const CAPTCHA_PROMPT: &str = concat!(
    "lucida requires you to complete a captcha!\n\n",
    "1. Open a new tab in your browser\n",
    "2. Open DevTools using F12 or Ctrl+Shift+I\n",
    "3. Select the Network tab\n",
    "4. Go to https://lucida.to/\n",
    "5. Complete the captcha\n",
    "6. Select one of the requests to lucida.to\n",
    "7. In the Request Headers section locate the Cookie and User-Agent headers\n",
    "8. Run the command again with two more arguments:\n",
    "  - set --cf-clearance to the value of the cf_clearance cookie from the Cookie header\n",
    "  - set --user-agent argument to the value of the User-Agent header; make sure to quote it!"
);

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    let mut urls = cli.urls;

    for file in cli.file {
        let mut lines = BufReader::new(File::open(file).await.unwrap()).lines();

        while let Some(line) = lines.next_line().await.unwrap() {
            urls.push(line);
        }
    }

    urls.reverse();

    if urls.is_empty() {
        eprintln!("no URLs to download");
        return ExitCode::FAILURE;
    }

    let urls_len = urls.len();

    let client = {
        let mut client = ClientBuilder::new();

        if let Some(user_agent) = &cli.user_agent {
            client = client.user_agent(user_agent);
        }

        if let Some(cf_clearance) = &cli.cf_clearance {
            client = client.default_headers(HeaderMap::from_iter([(
                COOKIE,
                format!("cf_clearance={cf_clearance}").try_into().unwrap(),
            )]));
        }

        client.build().unwrap()
    };

    match requests::check_availability(&client).await {
        Availability::Available => (),
        Availability::Captcha => {
            if cli.cf_clearance.is_some() && cli.user_agent.is_some() {
                eprintln!(
                    "Your cf_clearance cookie and User-Agent header weren't accepted. They might be stale"
                );
            } else {
                eprintln!("{CAPTCHA_PROMPT}");
            }

            return ExitCode::FAILURE;
        }
        Availability::Unavailable => {
            eprintln!("lucida seems to be unavailable right now. Visit the website: {BASE_URL}");
            return ExitCode::FAILURE;
        }
    }

    eprintln!("downloading {urls_len} albums");

    let urls = Arc::new(Mutex::new(urls));
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();
    let worker_count = cli.album_workers.min(urls_len);

    eprintln!("spawning {worker_count} album workers");

    tokio::spawn(async move {
        signal::ctrl_c().await.unwrap();
        running_clone.store(false, Ordering::Relaxed);
        eprintln!("Stopping gracefully");
        signal::ctrl_c().await.unwrap();
        process::exit(1);
    });

    let output = cli.output.unwrap_or_else(|| env::current_dir().unwrap());

    for result in future::join_all((1..=worker_count).map(|album_worker| {
        tokio::spawn(workers::run_album_worker(
            client.clone(),
            urls.clone(),
            output.clone(),
            cli.force,
            cli.group_singles,
            cli.album_year,
            cli.flatten_directories,
            DownloadConfig {
                country: cli.country.clone(),
                metadata: !cli.no_metadata,
                private: cli.private,
            },
            cli.track_workers,
            SkipConfig {
                tracks: cli.skip_tracks,
                cover: cli.skip_cover,
            },
            running.clone(),
            album_worker,
        ))
    }))
    .await
    {
        result.unwrap();
    }

    eprintln!("finished!");
    ExitCode::SUCCESS
}
