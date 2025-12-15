use crate::config::Config;
use anyhow::{anyhow, Context, Result};
use reqwest::blocking::Client;
use serde_json::Value;
use std::io::{self, Write};

const LASTFM_API_URL: &str = "https://ws.audioscrobbler.com/2.0/";

pub fn ensure_lastfm_session_key(config: &mut Config) -> Result<()> {
    let lastfm = match config.lastfm.as_mut() {
        Some(cfg) => cfg,
        None => return Ok(()),
    };

    if !lastfm.enabled {
        return Ok(());
    }

    if !lastfm.session_key.trim().is_empty() {
        return Ok(());
    }

    if lastfm.api_key.trim().is_empty() || lastfm.api_secret.trim().is_empty() {
        eprintln!("Last.fm scrobbling is enabled but `api_key`/`api_secret` are empty.");
        eprintln!("Please set valid credentials in your config before launching the app.");
        return Ok(());
    }

    println!("Last.fm scrobbling is enabled but no session key was found.");
    println!("Impulse will guide you through generating a session key.");

    let client = Client::builder()
        .user_agent("Impulse/0.1")
        .build()
        .context("Failed to build HTTP client for Last.fm authentication")?;

    let token = fetch_token(&client, &lastfm.api_key)?;
    let auth_url = format!(
        "https://www.last.fm/api/auth/?api_key={}&token={}",
        lastfm.api_key, token
    );
    println!();
    println!("Please authorize Impulse with Last.fm by visiting:");
    println!("{}", auth_url);

    if webbrowser::open(&auth_url).is_ok() {
        println!("Opened the URL in your default browser.");
    } else {
        println!("Unable to launch a browser automatically — copy the URL above manually.");
    }

    print!("Press Enter once you've allowed the application...");
    io::stdout().flush()?;
    io::stdin().read_line(&mut String::new())?;

    let session_key = fetch_session_key(&client, &lastfm.api_key, &lastfm.api_secret, &token)?;
    lastfm.session_key = session_key;
    config
        .save()
        .context("Failed to save configuration with the Last.fm session key")?;

    println!("Saved session key to config — Last.fm scrobbling is now ready.");

    Ok(())
}

fn fetch_token(client: &Client, api_key: &str) -> Result<String> {
    let response = client
        .get(LASTFM_API_URL)
        .query(&[
            ("method", "auth.gettoken"),
            ("api_key", api_key),
            ("format", "json"),
        ])
        .send()
        .context("Failed to request Last.fm token")?
        .error_for_status()
        .context("Last.fm token endpoint returned an error status")?;

    let value: Value = response
        .json()
        .context("Failed to parse Last.fm token response")?;

    if let Some(err) = lastfm_api_error(&value) {
        return Err(anyhow!(err));
    }

    value
        .get("token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("Last.fm token response did not include a token"))
}

fn fetch_session_key(
    client: &Client,
    api_key: &str,
    api_secret: &str,
    token: &str,
) -> Result<String> {
    let api_sig = format!(
        "{:x}",
        md5::compute(format!(
            "api_key{}methodauth.getSessiontoken{}{}",
            api_key, token, api_secret
        ))
    );

    let response = client
        .post(LASTFM_API_URL)
        .form(&[
            ("method", "auth.getSession"),
            ("api_key", api_key),
            ("token", token),
            ("api_sig", &api_sig),
            ("format", "json"),
        ])
        .send()
        .context("Failed to request Last.fm session")?
        .error_for_status()
        .context("Last.fm session endpoint returned an error status")?;

    let value: Value = response
        .json()
        .context("Failed to parse Last.fm session response")?;

    if let Some(err) = lastfm_api_error(&value) {
        return Err(anyhow!(err));
    }

    value
        .get("session")
        .and_then(|session| session.get("key"))
        .and_then(|key| key.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("Last.fm session response did not include a key"))
}

fn lastfm_api_error(value: &Value) -> Option<String> {
    value.get("error").map(|err| {
        let msg = value
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("No error message provided");
        match err.as_i64() {
            Some(code) => format!("Last.fm API error {}: {}", code, msg),
            None => format!("Last.fm API error: {}", msg),
        }
    })
}
