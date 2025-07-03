use route_recognizer::Router;
use serde::Deserialize;
use std::env;
use std::fs;
use std::process;

#[derive(Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum ValidatorResponse {
    Ok,
    Error { error: String },
}

struct MatchedRoute {
    endpoint: String,
}

enum RouteTarget {
    CardanoOverview,
    Noop,
}

fn match_route(path: &str) -> Option<MatchedRoute> {
    let mut router = Router::<RouteTarget>::new();
    router.add(
        "collections/cardano/:policy_id/overview.toml",
        RouteTarget::CardanoOverview,
    );
    router.add("tools/validator/Cargo.toml", RouteTarget::Noop);

    let matched = router.recognize(path).ok()?;
    let policy_id = matched.params().find("policy_id")?.to_string();

    match matched.handler() {
        RouteTarget::CardanoOverview => Some(format!(
            "https://curator.hodlcroft.net/validate/cardano/{}",
            policy_id
        )),
        _ => None,
    }
    .map(|endpoint| MatchedRoute { endpoint })
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!("Usage: validator <file1.toml> <file2.toml> ...");
        process::exit(1);
    }

    let mut errored = false;

    for file in &args {
        let endpoint = match match_route(file) {
            Some(m) => m.endpoint,
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
                    Ok(ValidatorResponse::Ok) => {
                        println!("✅ {} is valid", file);
                    }
                    Ok(ValidatorResponse::Error { error }) => {
                        eprintln!("❌ {} failed validation ({}):", file, error);
                    }
                    Err(_) => {
                        eprintln!("X {} invalidator validator response", file);
                    }
                }
            }
            Err(_) => {
                eprintln!("X {} validator request failed", file)
            }
        }
    }

    if errored {
        process::exit(1);
    }
}
