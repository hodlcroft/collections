use clap::{Parser, Subcommand};
use route_recognizer::Router;
use serde::Deserialize;
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
        /// Paths to TOML files to validate
        #[arg(required = true)]
        files: Vec<String>,
    },
    /// Submit updated TOML configs
    Update {
        /// Paths to TOML files to submit (must be routable)
        #[arg(required = true)]
        files: Vec<String>,
    },
}

#[derive(Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum ServiceResponse {
    Ok,
    Error { error: String },
}

#[derive(Clone)]
enum Action {
    Validate,
    Update,
}

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

fn get_endpoint_path(action: &Action, target: RouteTarget) -> Option<String> {
    match (action, target) {
        (Action::Validate, RouteTarget::CardanoOverview { policy_id }) => {
            Some(format!("/validate/cardano/{}", policy_id))
        }
        (Action::Update, RouteTarget::CardanoOverview { policy_id }) => {
            Some(format!("/update/cardano/{}", policy_id))
        }
    }
}

fn handle_validate(file: &str, endpoint: &str) -> Result<(), String> {
    let content = fs::read_to_string(file).map_err(|e| format!("read error: {e}"))?;

    let client = reqwest::blocking::Client::new();
    let res = client
        .post(endpoint)
        .header("Content-Type", "text/plain")
        .body(content)
        .send()
        .map_err(|e| format!("request error: {e}"))?;

    let body = res.text().unwrap_or_default();
    match serde_json::from_str::<ServiceResponse>(&body) {
        Ok(ServiceResponse::Ok) => {
            println!("✅ {} is valid", file);
            Ok(())
        }
        Ok(ServiceResponse::Error { error }) => Err(format!("❌ {} failed: {error}", file)),
        Err(_) => Err(format!("❌ {} invalid response", file)),
    }
}

fn handle_update(file: &str, endpoint: &str) -> Result<(), String> {
    let config = fs::read_to_string(file).map_err(|e| format!("read error: {e}"))?;
    let dir = Path::new(file).parent().unwrap_or_else(|| Path::new("."));
    let thumb = dir.join("thumbnail.png");
    let banner = dir.join("banner.jpg");

    let mut form = reqwest::blocking::multipart::Form::new().text("config", config);

    if thumb.exists() {
        form = form
            .file("thumbnail", &thumb)
            .map_err(|e| format!("thumb error: {e}"))?;
    }

    if banner.exists() {
        form = form
            .file("banner", &banner)
            .map_err(|e| format!("banner error: {e}"))?;
    }

    let client = reqwest::blocking::Client::new();
    let res = client
        .put(endpoint)
        .multipart(form)
        .send()
        .map_err(|e| format!("request error: {e}"))?;

    let body = res.text().unwrap_or_default();
    match serde_json::from_str::<ServiceResponse>(&body) {
        Ok(ServiceResponse::Ok) => {
            println!("📦 {} submitted", file);
            Ok(())
        }
        Ok(ServiceResponse::Error { error }) => Err(format!("❌ {} failed: {error}", file)),
        Err(_) => Err(format!("❌ {} invalid response", file)),
    }
}

fn main() {
    let cli = Cli::parse();
    let (action, files) = match cli.command {
        Commands::Validate { files } => (Action::Validate, files),
        Commands::Update { files } => (Action::Update, files),
    };

    let mut errored = false;

    for file in &files {
        let route = match_route(file);
        let endpoint_path = route.and_then(|r| get_endpoint_path(&action, r));
        let endpoint = match endpoint_path {
            Some(p) => format!("https://curator.hodlcroft.net{}", p),
            None => {
                eprintln!("⚠️  Skipping {} (no route)", file);
                continue;
            }
        };

        let result = match action {
            Action::Validate => handle_validate(file, &endpoint),
            Action::Update => handle_update(file, &endpoint),
        };

        if let Err(msg) = result {
            eprintln!("{msg}");
            errored = true;
        }
    }

    if errored {
        process::exit(1);
    }
}
