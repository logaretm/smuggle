//! The interception process.
//!
//! While it runs, `/etc/hosts` points the configured registry hosts at a
//! loopback address this process listens on, and it terminates TLS for them
//! using the CA installed by `smuggle setup`. Every request is currently
//! forwarded upstream unchanged; hijacking specific packages builds on top of
//! this.
//!
//! The redirect is owned by this process. It goes in at startup and comes out
//! on every exit path, so nothing is intercepted once the process is gone.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use std::task::{Context, Poll};

use console::style;
use http_body_util::BodyExt;
use hyper::body::Incoming;
use hyper::{Request, Response};
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::connect::dns::Name;
use hyper_util::rt::{TokioExecutor, TokioIo};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;

use super::{Registry, ca, hijack, hosts, loopback};
use crate::store;

/// Hop-by-hop headers, which must not be forwarded across a proxy hop.
/// See RFC 9110 section 7.6.1.
const HOP_BY_HOP: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];

type ProxyBody = http_body_util::combinators::BoxBody<bytes::Bytes, hyper::Error>;

type UpstreamClient = hyper_util::client::legacy::Client<
    hyper_rustls::HttpsConnector<HttpConnector<PinnedResolver>>,
    ProxyBody,
>;

pub struct Config {
    pub listen_ip: IpAddr,
    pub port: u16,
    pub registries: Vec<super::Registry>,
    /// Listen without editing `/etc/hosts`. Nothing is intercepted, so this is
    /// only useful for pointing a client at the proxy explicitly.
    pub no_redirect: bool,
    /// Log every request and every connection error.
    pub verbose: bool,
    /// Packages answered from the local store instead of upstream.
    pub hijack: Vec<String>,
}

/// Run the proxy in the foreground until interrupted. Requires root, both to
/// bind :443 and to edit `/etc/hosts`.
pub fn run(config: Config) -> Result<(), String> {
    let needs_root =
        !config.no_redirect || config.port < 1024 || !loopback::is_configured(config.listen_ip);
    if needs_root && unsafe { libc::geteuid() } != 0 {
        return Err("the proxy needs root to bind :443 and edit /etc/hosts".into());
    }
    if !ca::exists() {
        return Err("no local CA found — run `smuggle setup` first".into());
    }

    // Resolve upstreams before the redirect exists, otherwise these lookups
    // would come straight back to us.
    let mut registry_hosts: Vec<String> =
        config.registries.iter().map(|r| r.host.clone()).collect();
    registry_hosts.sort();
    registry_hosts.dedup();
    let upstreams = resolve_upstreams(&registry_hosts)?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("failed to start the async runtime: {e}"))?;

    // The listen address needs to exist on lo0 before we can bind it.
    let owns_alias = loopback::add_alias(config.listen_ip)?;

    let result = runtime.block_on(serve(&config, &registry_hosts, upstreams));

    if owns_alias {
        if let Err(e) = loopback::remove_alias(config.listen_ip) {
            let _ = cliclack::log::warning(format!("failed to remove the lo0 alias: {e}"));
        }
    }

    // Always remove the redirect, whether we exited cleanly or not.
    if !config.no_redirect {
        if let Err(e) = hosts::remove() {
            let _ =
                cliclack::log::warning(format!("failed to remove the /etc/hosts redirect: {e}"));
        }
    }

    result
}

fn resolve_upstreams(hosts: &[String]) -> Result<HashMap<String, Vec<SocketAddr>>, String> {
    let mut map = HashMap::new();

    for host in hosts {
        let addrs: Vec<SocketAddr> = (host.as_str(), 443)
            .to_socket_addrs()
            .map_err(|e| format!("failed to resolve {host}: {e}"))?
            .collect();

        if addrs.is_empty() {
            return Err(format!("{host} resolved to no addresses"));
        }
        if addrs.iter().any(|a| a.ip().is_loopback()) {
            return Err(format!(
                "{host} already resolves to loopback — run `smuggle cleanup` to clear a stale redirect"
            ));
        }
        map.insert(host.clone(), addrs);
    }

    Ok(map)
}

async fn serve(
    config: &Config,
    registry_hosts: &[String],
    upstreams: HashMap<String, Vec<SocketAddr>>,
) -> Result<(), String> {
    let addr = SocketAddr::new(config.listen_ip, config.port);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("failed to bind {addr}: {e}"))?;

    let tls = Arc::new(tls_acceptor()?);
    let client = Arc::new(upstream_client(upstreams));
    let hijacked: Arc<std::collections::HashSet<String>> =
        Arc::new(config.hijack.iter().cloned().collect());
    let mounts: Arc<HashMap<String, Registry>> = Arc::new(
        config
            .registries
            .iter()
            .map(|r| (r.host.clone(), r.clone()))
            .collect(),
    );

    if config.no_redirect {
        let _ = cliclack::log::info(format!(
            "Serving {} on {} without a redirect — nothing is intercepted",
            style(registry_hosts.join(", ")).cyan(),
            style(addr).dim(),
        ));
    } else {
        hosts::install(
            std::process::id() as i32,
            &config.listen_ip.to_string(),
            registry_hosts,
        )?;
        let _ = cliclack::log::success(format!(
            "Intercepting {} on {}",
            style(registry_hosts.join(", ")).cyan(),
            style(addr).dim(),
        ));
    }

    let verbose = config.verbose;
    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            _ = &mut shutdown => break,
            accepted = listener.accept() => {
                let Ok((stream, _)) = accepted else { continue };
                let tls = tls.clone();
                let client = client.clone();
                let hijacked = hijacked.clone();
                let mounts = mounts.clone();
                tokio::spawn(async move {
                    if let Err(e) =
                        handle_connection(stream, tls, client, hijacked, mounts, verbose).await
                    {
                        if verbose {
                            let _ = cliclack::log::warning(e);
                        }
                    }
                });
            }
        }
    }

    Ok(())
}

async fn shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};

    let mut term = match signal(SignalKind::terminate()) {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut int = match signal(SignalKind::interrupt()) {
        Ok(s) => s,
        Err(_) => return,
    };

    tokio::select! {
        _ = term.recv() => {}
        _ = int.recv() => {}
    }
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    tls: Arc<tokio_rustls::TlsAcceptor>,
    client: Arc<UpstreamClient>,
    hijacked: Arc<std::collections::HashSet<String>>,
    mounts: Arc<HashMap<String, Registry>>,
    verbose: bool,
) -> Result<(), String> {
    let stream = tls
        .accept(stream)
        .await
        .map_err(|e| format!("TLS handshake failed: {e}"))?;

    // The name the client asked for over SNI is the authority we forward to.
    // The Host header is client-controlled and carries the port we listen on,
    // not the port the real registry serves.
    let sni = stream
        .get_ref()
        .1
        .server_name()
        .ok_or("client connected without SNI")?
        .to_string();

    let service = hyper::service::service_fn(move |req: Request<Incoming>| {
        let client = client.clone();
        let sni = sni.clone();
        let hijacked = hijacked.clone();
        let mounts = mounts.clone();
        async move { forward(req, sni, client, hijacked, mounts, verbose).await }
    });

    hyper::server::conn::http1::Builder::new()
        .serve_connection(TokioIo::new(stream), service)
        .await
        .map_err(|e| format!("connection error: {e}"))
}

/// Pass a request through to the real registry unchanged.
async fn forward(
    req: Request<Incoming>,
    host: String,
    client: Arc<UpstreamClient>,
    hijacked: Arc<std::collections::HashSet<String>>,
    mounts: Arc<HashMap<String, Registry>>,
    verbose: bool,
) -> Result<Response<ProxyBody>, hyper::Error> {
    let path = req
        .uri()
        .path_and_query()
        .map(|p| p.as_str())
        .unwrap_or("/")
        .to_string();

    // Registries mounted under a path serve packages below it, so the mount
    // has to come off before the rest can be read as a package route.
    let route_path = mounts
        .get(&host)
        .and_then(|registry| registry.strip_prefix(&path));

    // Decide up front whether this request belongs to a package we answer for.
    let target = match route_path.map_or(hijack::Route::Other, hijack::parse_route) {
        hijack::Route::Tarball(name) if hijacked.contains(&name) => Some((name, Kind::Tarball)),
        hijack::Route::Packument(name) if hijacked.contains(&name) => Some((name, Kind::Packument)),
        hijack::Route::Manifest(name) if hijacked.contains(&name) => Some((name, Kind::Manifest)),
        _ => None,
    };

    // A tarball is served straight from the store without touching the network,
    // whatever version the client asked for.
    if let Some((name, Kind::Tarball)) = &target {
        return Ok(match store::load_tarball(name) {
            Ok(tarball) => {
                let _ = cliclack::log::success(format!(
                    "served {} from the store ({} bytes)",
                    style(name).cyan(),
                    tarball.len(),
                ));
                tarball_response(tarball)
            }
            Err(e) => bad_gateway(&format!("no packed tarball for {name}: {e}")),
        });
    }

    let (mut parts, body) = req.into_parts();
    let Ok(uri) = format!("https://{host}{path}").parse() else {
        return Ok(bad_gateway(&format!(
            "could not build an upstream URI for {host}{path}"
        )));
    };
    parts.uri = uri;

    // Rewrite Host to match, since the client's carries our listen port.
    if let Ok(value) = hyper::header::HeaderValue::from_str(&host) {
        parts.headers.insert(hyper::header::HOST, value);
    }

    // Whether the client sent a body has to be read before the framing headers
    // are stripped below.
    let has_body = parts.headers.contains_key(hyper::header::CONTENT_LENGTH)
        || parts.headers.contains_key(hyper::header::TRANSFER_ENCODING);

    for name in HOP_BY_HOP {
        parts.headers.remove(*name);
    }

    // A forwarded `Incoming` reports an unknown length, which makes hyper send
    // the upstream request chunked. Registries behind Cloudflare stall on a
    // chunked GET, so bodyless requests must carry a genuinely empty body.
    let body: ProxyBody = if has_body { body.boxed() } else { empty_body() };

    // Rewriting needs to read the JSON, so ask upstream not to compress it.
    if target.is_some() {
        parts.headers.insert(
            hyper::header::ACCEPT_ENCODING,
            hyper::header::HeaderValue::from_static("identity"),
        );
    }

    if verbose {
        let _ = cliclack::log::remark(format!("-> {} {}", parts.method, parts.uri));
    }

    match client.request(Request::from_parts(parts, body)).await {
        Ok(upstream) => {
            if verbose {
                let _ = cliclack::log::remark(format!("<- {} {host}{path}", upstream.status()));
            }
            let (mut parts, body) = upstream.into_parts();
            for name in HOP_BY_HOP {
                parts.headers.remove(*name);
            }

            match target {
                Some((name, kind)) if parts.status.is_success() => {
                    Ok(rewrite_response(parts, body, &name, kind).await)
                }
                _ => Ok(Response::from_parts(parts, body.boxed())),
            }
        }
        Err(e) => Ok(bad_gateway(&format!("upstream request failed: {e}"))),
    }
}

/// Which flavour of document a hijacked request is asking for.
#[derive(Clone, Copy)]
enum Kind {
    Packument,
    Manifest,
    Tarball,
}

/// Replace the integrity in an upstream packument or manifest with the hashes
/// of the tarball we will serve for it.
async fn rewrite_response(
    mut parts: hyper::http::response::Parts,
    body: Incoming,
    name: &str,
    kind: Kind,
) -> Response<ProxyBody> {
    let tarball = match store::load_tarball(name) {
        Ok(t) => t,
        Err(e) => return bad_gateway(&format!("no packed tarball for {name}: {e}")),
    };

    let original = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => return bad_gateway(&format!("could not read the upstream body: {e}")),
    };

    let rewritten = match kind {
        Kind::Packument => hijack::rewrite_packument(&original, &tarball),
        Kind::Manifest => hijack::rewrite_manifest(&original, &tarball),
        // Tarballs never reach here; they are served before the upstream call.
        Kind::Tarball => Ok(original.to_vec()),
    };

    let rewritten = match rewritten {
        Ok(bytes) => bytes,
        Err(e) => return bad_gateway(&format!("could not rewrite the {name} document: {e}")),
    };

    let _ = cliclack::log::success(format!(
        "rewrote {} integrity for {}",
        match kind {
            Kind::Packument => "packument",
            Kind::Manifest => "manifest",
            Kind::Tarball => "tarball",
        },
        style(name).cyan(),
    ));

    // Re-encoding changes the length, and any upstream encoding no longer
    // describes the body we are sending.
    parts.headers.remove(hyper::header::CONTENT_ENCODING);
    parts.headers.remove(hyper::header::CONTENT_LENGTH);
    parts.headers.remove(hyper::header::ETAG);

    Response::from_parts(parts, full_body(rewritten))
}

fn tarball_response(tarball: Vec<u8>) -> Response<ProxyBody> {
    Response::builder()
        .status(hyper::StatusCode::OK)
        .header(hyper::header::CONTENT_TYPE, "application/octet-stream")
        .body(full_body(tarball))
        .expect("tarball response is well-formed")
}

fn full_body(bytes: Vec<u8>) -> ProxyBody {
    http_body_util::Full::new(bytes::Bytes::from(bytes))
        .map_err(|never| match never {})
        .boxed()
}

fn bad_gateway(message: &str) -> Response<ProxyBody> {
    Response::builder()
        .status(hyper::StatusCode::BAD_GATEWAY)
        .body(full_body(message.as_bytes().to_vec()))
        .expect("bad gateway response is well-formed")
}

fn empty_body() -> ProxyBody {
    http_body_util::Empty::<bytes::Bytes>::new()
        .map_err(|never| match never {})
        .boxed()
}

fn upstream_client(upstreams: HashMap<String, Vec<SocketAddr>>) -> UpstreamClient {
    let mut http = HttpConnector::new_with_resolver(PinnedResolver {
        addrs: Arc::new(upstreams),
    });
    http.enforce_http(false);
    // Without this a request to an unreachable upstream hangs the client
    // rather than failing with a diagnosable error.
    http.set_connect_timeout(Some(std::time::Duration::from_secs(10)));

    let tls = rustls::ClientConfig::builder()
        .with_root_certificates(rustls::RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
        })
        .with_no_client_auth();

    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_tls_config(tls)
        .https_only()
        .enable_http1()
        .wrap_connector(http);

    hyper_util::client::legacy::Client::builder(TokioExecutor::new()).build(https)
}

/// Resolves the intercepted hostnames to the addresses we looked up before
/// installing the redirect. Going through the system resolver here would send
/// us back to our own listener.
#[derive(Clone)]
struct PinnedResolver {
    addrs: Arc<HashMap<String, Vec<SocketAddr>>>,
}

impl tower_service::Service<Name> for PinnedResolver {
    type Response = std::vec::IntoIter<SocketAddr>;
    type Error = std::io::Error;
    type Future = std::future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, name: Name) -> Self::Future {
        let result = match self.addrs.get(name.as_str()) {
            Some(addrs) => Ok(addrs.clone().into_iter()),
            None => Err(std::io::Error::other(format!(
                "{} is not an intercepted host",
                name.as_str()
            ))),
        };
        std::future::ready(result)
    }
}

fn tls_acceptor() -> Result<tokio_rustls::TlsAcceptor, String> {
    let signer = ca::LeafSigner::load()?;

    let mut config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_cert_resolver(Arc::new(CertResolver {
            signer,
            cache: std::sync::Mutex::new(HashMap::new()),
        }));

    // Only offer HTTP/1.1 so clients never negotiate h2, which this proxy does
    // not speak.
    config.alpn_protocols = vec![b"http/1.1".to_vec()];

    Ok(tokio_rustls::TlsAcceptor::from(Arc::new(config)))
}

/// Mints a leaf certificate per SNI name on first use.
struct CertResolver {
    signer: ca::LeafSigner,
    cache: std::sync::Mutex<HashMap<String, Arc<CertifiedKey>>>,
}

impl std::fmt::Debug for CertResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CertResolver")
    }
}

impl ResolvesServerCert for CertResolver {
    fn resolve(&self, hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        let host = hello.server_name()?.to_string();

        if let Some(hit) = self.cache.lock().unwrap().get(&host) {
            return Some(hit.clone());
        }

        let leaf = self.signer.leaf_for(&host).ok()?;
        let certified = Arc::new(certified_key(&leaf)?);

        self.cache.lock().unwrap().insert(host, certified.clone());
        Some(certified)
    }
}

fn certified_key(leaf: &ca::Leaf) -> Option<CertifiedKey> {
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut leaf.cert_pem.as_bytes())
        .collect::<Result<_, _>>()
        .ok()?;

    let key: PrivateKeyDer<'static> =
        rustls_pemfile::private_key(&mut leaf.key_pem.as_bytes()).ok()??;

    let signing_key = rustls::crypto::ring::sign::any_supported_type(&key).ok()?;
    Some(CertifiedKey::new(certs, signing_key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_service::Service;

    fn resolver() -> PinnedResolver {
        let mut addrs = HashMap::new();
        addrs.insert(
            "registry.npmjs.org".to_string(),
            vec!["104.16.8.34:443".parse().unwrap()],
        );
        PinnedResolver {
            addrs: Arc::new(addrs),
        }
    }

    #[tokio::test]
    async fn resolves_a_pinned_host_to_its_prelookup_address() {
        let name: Name = "registry.npmjs.org".parse().unwrap();
        let resolved: Vec<SocketAddr> = resolver().call(name).await.unwrap().collect();
        assert_eq!(resolved, ["104.16.8.34:443".parse::<SocketAddr>().unwrap()]);
    }

    #[tokio::test]
    async fn refuses_hosts_it_was_not_given() {
        // Falling back to the system resolver here would route us at our own
        // listener, since /etc/hosts points intercepted names at it.
        let name: Name = "example.com".parse().unwrap();
        assert!(resolver().call(name).await.is_err());
    }
}
