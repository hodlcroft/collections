use clap::{Parser, Subcommand};
use route_recognizer::Router;
use serde::Deserialize;
use std::fs;
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
        /// Paths to TOML files to submit
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
    // Step 1: route pattern → string label
    let mut router = Router::<&str>::new();
    router.add(
        "collections/cardano/:policy_id/overview.toml",
        "cardano_overview",
    );

    let matched = router.recognize(path).ok()?;
    let policy_id = matched.params().find("policy_id")?.to_string();

    // Step 2: handler label → RouteTarget construction
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
        _ => None,
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
        let endpoint_path = match_route(file).and_then(|target| get_endpoint_path(&action, target));

        let endpoint = match endpoint_path {
            Some(path) => format!("https://curator.hodlcroft.net{}", path),
            None => {
                eprintln!("⚠️  Skipping {}: no route matched", file);
                continue;
            }
        };

        let content = match fs::read_to_string(file) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("❌ Failed to read file {}: {}", file, e);
                errored = true;
                continue;
            }
        };

        let res = ureq::post(&endpoint)
            .set("Content-Type", "text/plain")
            .send_string(&content);

        match res {
            Ok(res) => {
                let body = res.into_string().unwrap_or_default();
                match serde_json::from_str(&body) {
                    Ok(ServiceResponse::Ok) => {
                        println!("✅ {} succeeded", file);
                    }
                    Ok(ServiceResponse::Error { error }) => {
                        eprintln!("❌ {} failed ({}):", file, error);
                        errored = true;
                    }
                    Err(_) => {
                        eprintln!("X {} invalid JSON from server", file);
                        errored = true;
                    }
                }
            }
            Err(_) => {
                eprintln!("X {} validator request failed ({})", file, endpoint);
                errored = true;
            }
        }
    }

    if errored {
        process::exit(1);
    }
}
