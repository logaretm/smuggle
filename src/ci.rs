use serde::Serialize;

/// Returns true when running in a CI environment.
/// Checks the `CI` env var, which is set by GitHub Actions, GitLab CI,
/// CircleCI, Travis, Jenkins, and most other CI providers.
pub fn is_ci() -> bool {
    std::env::var("CI")
        .map(|v| !v.is_empty() && v != "0" && v.to_lowercase() != "false")
        .unwrap_or(false)
}

/// NDJSON event types emitted in --json mode.
#[derive(Serialize)]
#[serde(tag = "event")]
pub enum Event<'a> {
    #[serde(rename = "publish")]
    Publish {
        package: &'a str,
        version: &'a str,
        status: Status,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<&'a str>,
    },
    #[serde(rename = "install")]
    Install {
        package: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        location: Option<&'a str>,
        status: Status,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<&'a str>,
    },
    #[serde(rename = "summary")]
    Summary {
        published: usize,
        installed: usize,
        failed: usize,
    },
    #[serde(rename = "error")]
    Error { message: &'a str },
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Error,
    Skipped,
}

/// Emit a single NDJSON line to stdout.
pub fn emit(event: &Event) {
    if let Ok(json) = serde_json::to_string(event) {
        println!("{json}");
    }
}
