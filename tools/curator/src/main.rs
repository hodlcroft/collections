use clap::{Parser, Subcommand};
use reqwest::blocking::Client;
use route_recognizer::Router;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process;

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate one or more TOML files
    Validate {
        #[arg(required = true)]
        files: Vec<String>,
    },
    /// Submit updated TOML config and optional images
    Update {
        #[arg(required = true)]
        files: Vec<String>,
    },
}

#[derive(Clone)]
enum Action {
    Validate,
    Update,
}

#[derive(Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum ServiceResponse {
    Ok,
    Error { error: String },
}

#[derive(Debug)]
enum RouteTarget {
    CardanoOverview { policy_id: String },
}

fn match_route(path: &str) -> Option<RouteTarget> {
    let mut router = Router::<&str>::new();
    router.add(
        "collections/cardano/:policy_id/overview.toml",
        "cardano_overview",
    );

    let matched = router.recognize(path).ok()?;
    let policy_id = matched.params().find("policy_id")?.to_string();

    match **matched.handler() {
        "cardano_overview" => Some(RouteTarget::CardanoOverview { policy_id }),
        _ => None,
    }
}

fn get_endpoints(action: &Action, target: &RouteTarget) -> HashMap<&'static str, String> {
    match (action, target) {
        (Action::Validate, RouteTarget::CardanoOverview { policy_id }) => {
            let mut map = HashMap::new();
            map.insert(
                "config",
                format!(
                    "https://curator.hodlcroft.net/validate/cardano/{}",
                    policy_id
                ),
            );
            map
        }
        (Action::Update, RouteTarget::CardanoOverview { policy_id }) => {
            let base = format!("https://curator.hodlcroft.net/update/cardano/{}", policy_id);
            let mut map = HashMap::new();
            map.insert("config", format!("{base}/config"));
            map.insert("thumbnail", format!("{base}/thumbnail"));
            map.insert("banner", format!("{base}/banner"));
            map
        }
    }
}

fn post_file(
    client: &Client,
    endpoint: &str,
    content: Vec<u8>,
    content_type: &str,
) -> Result<(), String> {
    let res = client
        .post(endpoint)
        .header("Content-Type", content_type)
        .body(content)
        .send()
        .map_err(|e| format!("request error: {e}"))?;

    let body = res.text().unwrap_or_default();
    match serde_json::from_str::<ServiceResponse>(&body) {
        Ok(ServiceResponse::Ok) => Ok(()),
        Ok(ServiceResponse::Error { error }) => Err(error),
        Err(_) => Err("invalid server response".into()),
    }
}

fn handle_validate(client: &Client, file: &str, endpoint: &str) -> Result<(), String> {
    let content = fs::read_to_string(file).map_err(|e| format!("read error: {e}"))?;
    post_file(client, endpoint, content.into_bytes(), "text/plain")
}

fn handle_update(
    client: &Client,
    file: &str,
    endpoints: &HashMap<&str, String>,
) -> Result<(), String> {
    let base = Path::new(file).parent().unwrap_or_else(|| Path::new("."));

    // Always send config
    let config = fs::read_to_string(file).map_err(|e| format!("read error: {e}"))?;
    post_file(
        client,
        &endpoints["config"],
        config.into_bytes(),
        "text/plain",
    )
    .map_err(|e| format!("config upload failed: {e}"))?;

    // Optional thumbnail.png
    let thumb_path = base.join("thumbnail.png");
    if thumb_path.exists() {
        let bytes = fs::read(&thumb_path).map_err(|e| format!("thumbnail read error: {e}"))?;
        post_file(client, &endpoints["thumbnail"], bytes, "image/png")
            .map_err(|e| format!("thumbnail upload failed: {e}"))?;
    }

    // Optional banner.jpg
    let banner_path = base.join("banner.jpg");
    if banner_path.exists() {
        let bytes = fs::read(&banner_path).map_err(|e| format!("banner read error: {e}"))?;
        post_file(client, &endpoints["banner"], bytes, "image/jpeg")
            .map_err(|e| format!("banner upload failed: {e}"))?;
    }

    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let (action, files) = match cli.command {
        Commands::Validate { files } => (Action::Validate, files),
        Commands::Update { files } => (Action::Update, files),
    };

    let client = Client::new();
    let mut errored = false;

    for file in &files {
        let route = match_route(file);
        let endpoints = route
            .as_ref()
            .map(|r| get_endpoints(&action, r))
            .unwrap_or_default();

        if endpoints.is_empty() {
            eprintln!("⚠️  Skipping {file} (no route matched)");
            continue;
        }

        let result = match action {
            Action::Validate => handle_validate(&client, file, &endpoints["config"]),
            Action::Update => handle_update(&client, file, &endpoints),
        };

        match result {
            Ok(()) => match action {
                Action::Validate => println!("✅ {file} is valid"),
                Action::Update => println!("📦 {file} submitted"),
            },
            Err(msg) => {
                eprintln!("❌ {file}: {msg}");
                errored = true;
            }
        }
    }

    if errored {
        process::exit(1);
    }
}
