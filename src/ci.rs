use serde::Serialize;
use std::time::Instant;

/// Returns true when running in a CI environment.
/// Checks the `CI` env var, which is set by GitHub Actions, GitLab CI,
/// CircleCI, Travis, Jenkins, and most other CI providers.
pub fn is_ci() -> bool {
    std::env::var("CI")
        .map(|v| !v.is_empty() && v != "0" && v.to_lowercase() != "false")
        .unwrap_or(false)
}

/// NDJSON event types emitted in --ci mode.
#[derive(Serialize, Clone)]
#[serde(tag = "event")]
pub enum Event<'a> {
    #[serde(rename = "publish")]
    Publish {
        package: &'a str,
        version: &'a str,
        status: Status,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
    },
    #[serde(rename = "summary")]
    Summary {
        published: usize,
        failed: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
    },
    #[serde(rename = "error")]
    Error { message: &'a str },
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Error,
}

/// Emit a single NDJSON line to stdout.
pub fn emit(event: &Event) {
    if let Ok(json) = serde_json::to_string(event) {
        println!("{json}");
    }
}

/// Returns milliseconds elapsed since `start`.
pub fn elapsed_ms(start: Instant) -> u64 {
    start.elapsed().as_millis() as u64
}

/// Collected results for writing a GitHub Actions job summary.
pub struct SummaryCollector {
    pub rows: Vec<SummaryRow>,
    pub start: Instant,
}

pub struct SummaryRow {
    pub action: String,
    pub package: String,
    pub version: String,
    pub status: String,
    pub duration_ms: u64,
}

impl SummaryCollector {
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            start: Instant::now(),
        }
    }

    pub fn push(
        &mut self,
        action: &str,
        package: &str,
        version: &str,
        status: &str,
        duration_ms: u64,
    ) {
        self.rows.push(SummaryRow {
            action: action.to_string(),
            package: package.to_string(),
            version: version.to_string(),
            status: status.to_string(),
            duration_ms,
        });
    }

    /// Write a markdown summary table to `$GITHUB_STEP_SUMMARY` if the env var is set.
    pub fn write_github_summary(&self) {
        let Some(path) = std::env::var("GITHUB_STEP_SUMMARY")
            .ok()
            .filter(|p| !p.is_empty())
        else {
            return;
        };

        let total_ms = elapsed_ms(self.start);
        let published = self
            .rows
            .iter()
            .filter(|r| r.action == "publish" && r.status == "ok")
            .count();
        let failed = self.rows.iter().filter(|r| r.status == "error").count();

        let mut md = String::new();
        md.push_str("### 📦 Smuggle Summary\n\n");
        md.push_str(&format!(
            "**{}** published, **{}** failed, completed in {}\n\n",
            published,
            failed,
            format_duration(total_ms)
        ));

        if !self.rows.is_empty() {
            md.push_str("| Action | Package | Version | Status | Duration |\n");
            md.push_str("|--------|---------|---------|--------|----------|\n");
            for row in &self.rows {
                let status_icon = match row.status.as_str() {
                    "ok" => "✅",
                    "error" => "❌",
                    _ => "❓",
                };
                md.push_str(&format!(
                    "| {} | `{}` | {} | {} | {} |\n",
                    row.action,
                    row.package,
                    row.version,
                    status_icon,
                    format_duration(row.duration_ms),
                ));
            }
        }

        // Append to the summary file (GitHub expects append, not overwrite)
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut f| {
                use std::io::Write;
                f.write_all(md.as_bytes())
            });
    }
}

fn format_duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.1}s", ms as f64 / 1000.0)
    }
}
