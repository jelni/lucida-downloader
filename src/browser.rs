use std::time::{Duration, Instant};

use chaser_oxide::cdp::browser_protocol::network::GetCookiesParams;
use chaser_oxide::{Browser, BrowserConfig, ChaserPage};
use futures::StreamExt;

pub async fn get_cf_clearance() -> Option<(String, String)> {
    // Use a temp directory as user data dir so we get a clean profile with no
    // stale cookies
    let tmp_dir = match tempfile::tempdir() {
        Ok(dir) => dir,
        Err(e) => {
            tracing::error!("failed to create temp directory: {e}");
            return None;
        }
    };

    let config = match BrowserConfig::builder()
        .no_sandbox()
        .user_data_dir(tmp_dir.path())
        .build()
    {
        Ok(config) => config,
        Err(e) => {
            tracing::error!("failed to build browser config: {e}");
            return None;
        }
    };

    let (mut browser, mut handler) = match Browser::launch(config).await {
        Ok(pair) => pair,
        Err(e) => {
            tracing::error!("failed to launch browser: {e}");
            return None;
        }
    };

    tokio::spawn(async move { while handler.next().await.is_some() {} });

    let page = match browser.new_page("about:blank").await {
        Ok(page) => page,
        Err(e) => {
            tracing::error!("failed to open browser page: {e}");
            return None;
        }
    };

    let chaser = ChaserPage::new(page);

    // Apply native OS/Chrome/RAM fingerprint for stealth
    if let Err(e) = chaser.apply_native_profile().await {
        tracing::error!("failed to apply native profile: {e}");
        return None;
    }

    if let Err(e) = chaser.goto("https://lucida.to").await {
        tracing::error!("failed to navigate to lucida.to: {e}");
        return None;
    }

    // Poll for the cf_clearance cookie (Cloudflare sets it after challenge
    // passes)
    let timeout = Duration::from_secs(120);
    let poll_interval = Duration::from_millis(2000);
    let start = Instant::now();

    let mut cf_clearance = String::new();
    loop {
        if start.elapsed() > timeout {
            tracing::error!("timed out waiting for cf_clearance cookie");
            return None;
        }

        let response = match chaser.raw_page().execute(GetCookiesParams::default()).await {
            Ok(response) => response,
            Err(e) => {
                tracing::warn!("failed to get cookies: {e}");
                tokio::time::sleep(poll_interval).await;
                continue;
            }
        };

        for cookie in &response.cookies {
            if cookie.name == "cf_clearance" {
                cf_clearance = cookie.value.clone();
            }
        }

        if !cf_clearance.is_empty() {
            break;
        }

        tokio::time::sleep(poll_interval).await;
    }

    // Extract the user-agent via CDP Browser.getVersion
    let version = match chaser
        .raw_page()
        .execute(chaser_oxide::cdp::browser_protocol::browser::GetVersionParams::default())
        .await
    {
        Ok(version) => version,
        Err(e) => {
            tracing::error!("failed to get browser version: {e}");
            return None;
        }
    };
    let user_agent = version.user_agent.clone();

    let _ = browser.close().await;

    Some((cf_clearance, user_agent))
}
