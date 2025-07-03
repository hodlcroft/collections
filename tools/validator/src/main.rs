use std::env;
use std::fs;
use std::process;

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum ValidatorResponse {
    Ok,
    Error { error: String },
}

fn route(file: &str) -> Option<&'static str> {
    if file.starts_with("template/collection/") {
        Some("https://api.hodlcroft.net/validate/collection")
    } else if file.starts_with("template/theme/") {
        Some("https://api.hodlcroft.net/validate/theme")
    } else {
        None
    }
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!("Usage: validate-toml <file1.toml> <file2.toml> ...");
        process::exit(1);
    }

    let mut errored = false;

    for file in &args {
        let endpoint = match route(file) {
            Some(url) => url,
            None => {
                eprintln!("❌ No route for file: {}", file);
                errored = true;
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

        let res = ureq::post(endpoint)
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
