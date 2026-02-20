pub mod server;

use anyhow::{bail, Result};
use std::time::Duration;

use crate::config::Config;
use server::CallbackServer;

/// Supported providers for browser-based auth.
const SUPPORTED: &[&str] = &["anthropic", "openai"];

pub async fn login(provider: &str) -> Result<()> {
    if !SUPPORTED.contains(&provider) {
        bail!(
            "Unknown provider '{}'. Supported: {}",
            provider,
            SUPPORTED.join(", ")
        );
    }

    let mut config = Config::load()?;

    // Check if already authenticated
    let already = match provider {
        "anthropic" => config.provider.anthropic.api_key.is_some(),
        "openai" => config.provider.openai.api_key.is_some(),
        _ => false,
    };
    if already {
        println!(
            "Already authenticated with {}. Run `openrust logout {}` first to re-authenticate.",
            provider_display(provider),
            provider
        );
        return Ok(());
    }

    // Bind the local server first so we have a port before opening the browser
    let srv = CallbackServer::bind().await?;
    let port = srv.port;
    let local_url = format!("http://127.0.0.1:{}", port);

    print_login_banner(provider, &local_url);

    // Open the browser to our local form
    if let Err(e) = open::that(&local_url) {
        println!(
            "  Could not open browser automatically: {e}\n  Open manually: {local_url}"
        );
    }

    println!("  Waiting for key submission in browser…");
    println!("  (Press Ctrl+C to cancel)");
    println!();

    // Wait for the user to submit their key via the browser form
    let key = tokio::time::timeout(Duration::from_secs(300), srv.serve(provider))
        .await
        .map_err(|_| anyhow::anyhow!("Login timed out after 5 minutes"))??;

    // Validate key format
    validate_key(provider, &key)?;

    // Persist to config
    match provider {
        "anthropic" => config.provider.anthropic.api_key = Some(key.clone()),
        "openai" => config.provider.openai.api_key = Some(key.clone()),
        _ => {}
    }
    config.save()?;

    println!("  {} API key saved to {}", provider_display(provider), Config::config_path().display());
    println!("  You can now run `openrust` to start chatting.");
    Ok(())
}

pub fn logout(provider: &str) -> Result<()> {
    if !SUPPORTED.contains(&provider) {
        bail!(
            "Unknown provider '{}'. Supported: {}",
            provider,
            SUPPORTED.join(", ")
        );
    }

    let mut config = Config::load()?;

    let had_key = match provider {
        "anthropic" => config.provider.anthropic.api_key.take().is_some(),
        "openai" => config.provider.openai.api_key.take().is_some(),
        _ => false,
    };

    config.save()?;

    if had_key {
        println!("Logged out of {}. API key removed from config.", provider_display(provider));
    } else {
        println!("{} was not logged in.", provider_display(provider));
    }

    Ok(())
}

fn validate_key(provider: &str, key: &str) -> Result<()> {
    match provider {
        "anthropic" => {
            if !key.starts_with("sk-ant-") {
                bail!(
                    "Invalid Anthropic API key format (expected 'sk-ant-…'). Got: {}…",
                    &key[..key.len().min(12)]
                );
            }
        }
        "openai" => {
            if !key.starts_with("sk-") {
                bail!(
                    "Invalid OpenAI API key format (expected 'sk-…'). Got: {}…",
                    &key[..key.len().min(12)]
                );
            }
        }
        _ => {}
    }
    Ok(())
}

fn provider_display(provider: &str) -> &str {
    match provider {
        "anthropic" => "Anthropic (Claude)",
        "openai" => "OpenAI",
        _ => provider,
    }
}

fn key_page_url(provider: &str) -> &str {
    match provider {
        "anthropic" => "https://console.anthropic.com/settings/keys",
        "openai" => "https://platform.openai.com/api-keys",
        _ => "",
    }
}

fn print_login_banner(provider: &str, local_url: &str) {
    println!();
    println!("  OpenRust – {} login", provider_display(provider));
    println!("  {}", "─".repeat(48));
    println!();
    println!("  A browser window will open at:");
    println!("    {}", local_url);
    println!();
    println!("  If the browser doesn't open automatically, visit:");
    println!("    {}", local_url);
    println!();
    println!("  You'll be asked to paste your API key from:");
    println!("    {}", key_page_url(provider));
    println!();
}
