use sha2::{Digest, Sha512};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;

pub struct RegistryPackage {
    pub name: String,
    pub version: String,
    pub tarball: Vec<u8>,
    pub dependencies: HashMap<String, String>,
}

pub struct Server {
    pub port: u16,
    packages: Arc<RwLock<HashMap<String, PackageState>>>,
    _handle: thread::JoinHandle<()>,
}

struct PackageState {
    name: String,
    version: String,
    revision: AtomicU64,
    tarball: Vec<u8>,
    integrity: String,
    dependencies: HashMap<String, String>,
}

impl PackageState {
    /// Returns a version string with an lpm revision suffix to bust caches.
    /// First publish: 1.0.0-smuggle.0, after first update: 1.0.0-smuggle.1, etc.
    fn versioned(&self) -> String {
        let rev = self.revision.load(Ordering::Relaxed);
        format!("{}-smuggle.{rev}", self.version)
    }
}

impl Server {
    pub fn update_tarball(&self, name: &str, tarball: Vec<u8>) {
        let integrity = compute_integrity(&tarball);
        if let Ok(mut packages) = self.packages.write() {
            if let Some(state) = packages.get_mut(name) {
                state.tarball = tarball;
                state.integrity = integrity;
                state.revision.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

fn compute_integrity(data: &[u8]) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let hash = Sha512::digest(data);
    format!("sha512-{}", STANDARD.encode(hash))
}

pub fn start(packages: Vec<RegistryPackage>) -> Result<Server, String> {
    let mut state_map = HashMap::new();
    for pkg in packages {
        let integrity = compute_integrity(&pkg.tarball);
        state_map.insert(
            pkg.name.clone(),
            PackageState {
                name: pkg.name,
                version: pkg.version,
                revision: AtomicU64::new(0),
                tarball: pkg.tarball,
                integrity,
                dependencies: pkg.dependencies,
            },
        );
    }

    let packages = Arc::new(RwLock::new(state_map));

    // Bind to port 0 to let the OS pick a free port
    let server = tiny_http::Server::http("127.0.0.1:0")
        .map_err(|e| format!("failed to start registry server: {e}"))?;

    let port = server
        .server_addr()
        .to_ip()
        .map(|addr| addr.port())
        .ok_or("failed to get server port")?;

    let packages_clone = packages.clone();

    let handle = thread::spawn(move || {
        serve(server, packages_clone);
    });

    Ok(Server {
        port,
        packages,
        _handle: handle,
    })
}

const UPSTREAM_REGISTRY: &str = "https://registry.npmjs.org";

fn serve(server: tiny_http::Server, packages: Arc<RwLock<HashMap<String, PackageState>>>) {
    for request in server.incoming_requests() {
        let url = request.url().to_string();
        let url = url.trim_start_matches('/');

        // Parse the request path
        // GET /<name> → package metadata
        // GET /<name>/-/<name>-<version>.tgz → tarball
        // GET /@scope/<name> → scoped package metadata
        // GET /@scope/<name>/-/@scope/<name>-<version>.tgz → scoped tarball

        let packages_guard = packages.read().unwrap();

        // Check if this is a tarball request (contains /-/)
        if let Some(pkg_name) = extract_tarball_request(url) {
            if let Some(state) = packages_guard.get(&pkg_name) {
                let response = tiny_http::Response::from_data(state.tarball.clone())
                    .with_header(
                        "Content-Type: application/octet-stream"
                            .parse::<tiny_http::Header>()
                            .unwrap(),
                    );
                let _ = request.respond(response);
                continue;
            }
        }

        // Check if this is a metadata request
        let pkg_name = decode_scoped_name(url);

        if let Some(state) = packages_guard.get(&pkg_name) {
            let metadata = build_metadata(state, server.server_addr().to_ip().unwrap().port());
            let response = tiny_http::Response::from_string(metadata).with_header(
                "Content-Type: application/json"
                    .parse::<tiny_http::Header>()
                    .unwrap(),
            );
            let _ = request.respond(response);
            continue;
        }

        // Not one of our packages — proxy to upstream
        drop(packages_guard);
        match proxy_to_upstream(url) {
            Ok((status, headers, body)) => {
                let mut response = tiny_http::Response::from_data(body).with_status_code(status);
                for header in headers {
                    response = response.with_header(header);
                }
                let _ = request.respond(response);
            }
            Err(_) => {
                let response = tiny_http::Response::from_string("not found")
                    .with_status_code(tiny_http::StatusCode(404));
                let _ = request.respond(response);
            }
        }
    }
}

fn extract_tarball_request(url: &str) -> Option<String> {
    // Pattern: <name>/-/<name>-<version>.tgz
    // For scoped: @scope/<name>/-/@scope/<name>-<version>.tgz
    let idx = url.find("/-/")?;
    let pkg_name = &url[..idx];
    Some(decode_scoped_name(pkg_name))
}

fn decode_scoped_name(url_path: &str) -> String {
    // URL might encode @ as %40 — decode it
    url_path.replace("%2f", "/").replace("%2F", "/").replace("%40", "@")
}

fn build_metadata(state: &PackageState, port: u16) -> String {
    let name = &state.name;
    let base_version = &state.version;
    let lpm_version = state.versioned();
    let integrity = &state.integrity;
    let tarball_name = tarball_filename(name, &lpm_version);
    let encoded_name = name.replace('/', "%2f");

    let deps_json = serde_json::to_string(&state.dependencies).unwrap_or_else(|_| "{}".into());

    // Serve the base version (matches semver ranges like ^1.0.0) but with our tarball.
    // Also serve the lpm-versioned entry so the lockfile records a unique version,
    // forcing re-fetch on subsequent installs.
    // dist-tags.latest points to the lpm version, but the base version also exists
    // so that range resolution works.
    format!(
        r#"{{
  "name": "{name}",
  "dist-tags": {{
    "latest": "{lpm_version}"
  }},
  "versions": {{
    "{base_version}": {{
      "name": "{name}",
      "version": "{base_version}",
      "dependencies": {deps_json},
      "dist": {{
        "tarball": "http://localhost:{port}/{encoded_name}/-/{tarball_name}",
        "integrity": "{integrity}"
      }}
    }},
    "{lpm_version}": {{
      "name": "{name}",
      "version": "{lpm_version}",
      "dependencies": {deps_json},
      "dist": {{
        "tarball": "http://localhost:{port}/{encoded_name}/-/{tarball_name}",
        "integrity": "{integrity}"
      }}
    }}
  }}
}}"#
    )
}

fn tarball_filename(name: &str, version: &str) -> String {
    // @scope/pkg → scope-pkg-version.tgz (npm convention)
    let clean_name = if let Some(rest) = name.strip_prefix('@') {
        rest.replace('/', "-")
    } else {
        name.to_string()
    };
    format!("{clean_name}-{version}.tgz")
}

fn proxy_to_upstream(path: &str) -> Result<(u16, Vec<tiny_http::Header>, Vec<u8>), String> {
    let url = format!("{UPSTREAM_REGISTRY}/{path}");

    // Use a simple blocking HTTP request via std
    // We'll shell out to curl for simplicity since we don't want to pull in reqwest
    let output = std::process::Command::new("curl")
        .args([
            "-sS",
            "--max-time",
            "30",
            "-H",
            "Accept: application/json",
            "-w",
            "\n%{http_code}",
            &url,
        ])
        .output()
        .map_err(|e| format!("curl failed: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.rsplitn(2, '\n').collect();

    if lines.len() < 2 {
        return Err("unexpected curl output".into());
    }

    let status: u16 = lines[0].trim().parse().unwrap_or(502);
    let body = lines[1].as_bytes().to_vec();

    let headers = vec![
        "Content-Type: application/json"
            .parse::<tiny_http::Header>()
            .unwrap(),
    ];

    Ok((status, headers, body))
}
