use std::env;
use std::fs;
use std::path::PathBuf;

use serde_json::Value;

use crate::refusal::RefusalPayload;

pub mod pull;
pub mod push;

pub use pull::handle_pull;
pub use push::handle_push;

const FABRIC_URL_ENV: &str = "EPISTEMIC_FABRIC_URL";

pub(crate) fn post_json(path: &str, body: &Value) -> Result<String, RefusalPayload> {
    let endpoint = endpoint(path)?;
    let response = ureq::post(&endpoint)
        .set("content-type", "application/json")
        .send_json(body.clone());

    map_response(response, &endpoint)
}

pub(crate) fn get_text(path: &str) -> Result<String, RefusalPayload> {
    let endpoint = endpoint(path)?;
    map_response(ureq::get(&endpoint).call(), &endpoint)
}

fn endpoint(path: &str) -> Result<String, RefusalPayload> {
    let base = resolve_fabric_url()?;
    Ok(format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    ))
}

fn resolve_fabric_url() -> Result<String, RefusalPayload> {
    if let Ok(value) = env::var(FABRIC_URL_ENV) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    let home = env::var("HOME").map_err(|error| {
        RefusalPayload::io(
            "$HOME".to_string(),
            format!("HOME environment variable unavailable: {error}"),
        )
    })?;
    let config_path = PathBuf::from(home).join(".epistemic").join("config.toml");
    let content = fs::read_to_string(&config_path).map_err(|error| {
        RefusalPayload::io(config_path.display().to_string(), error.to_string())
    })?;

    if let Some(url) = parse_fabric_url(&content) {
        return Ok(url);
    }

    Err(RefusalPayload::io(
        config_path.display().to_string(),
        "missing [fabric].url setting".to_string(),
    ))
}

fn parse_fabric_url(content: &str) -> Option<String> {
    let mut in_fabric_section = false;

    for raw_line in content.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            in_fabric_section = line == "[fabric]";
            continue;
        }

        if !in_fabric_section {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "url" {
            continue;
        }

        let trimmed = value.trim().trim_matches('"').trim_matches('\'').trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    None
}

fn map_response(
    response: Result<ureq::Response, ureq::Error>,
    endpoint: &str,
) -> Result<String, RefusalPayload> {
    match response {
        Ok(ok) => {
            let status = ok.status();
            let body = ok
                .into_string()
                .map_err(|error| RefusalPayload::io(endpoint.to_string(), error.to_string()))?;

            if (200..300).contains(&status) {
                Ok(body)
            } else {
                Err(RefusalPayload::io(
                    endpoint.to_string(),
                    format_http_error(status, &body),
                ))
            }
        }
        Err(ureq::Error::Status(status, response)) => {
            let body = response.into_string().unwrap_or_default();
            Err(RefusalPayload::io(
                endpoint.to_string(),
                format_http_error(status, &body),
            ))
        }
        Err(ureq::Error::Transport(error)) => {
            Err(RefusalPayload::io(endpoint.to_string(), error.to_string()))
        }
    }
}

fn format_http_error(status: u16, body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        format!("HTTP {status}")
    } else {
        format!("HTTP {status}: {trimmed}")
    }
}

#[cfg(test)]
mod tests {
    use super::parse_fabric_url;

    #[test]
    fn parse_fabric_url_reads_value_from_fabric_section() {
        let toml = r#"
[other]
url = "https://ignored.example"

[fabric]
url = "https://fabric.example"
"#;

        assert_eq!(
            parse_fabric_url(toml).as_deref(),
            Some("https://fabric.example")
        );
    }
}
