//! Registry request rewriting.
//!
//! The proxy answers for a hijacked package with the tarball from the local
//! store, whatever version was asked for. Packuments are fetched from upstream
//! and rewritten so the integrity the client checks matches the bytes we serve.

use serde_json::Value;

/// What a registry request path is asking for.
#[derive(Debug, PartialEq, Eq)]
pub enum Route {
    /// The full version list for a package.
    Packument(String),
    /// A single version's manifest.
    Manifest(String),
    /// A tarball. The version in the filename is ignored.
    Tarball(String),
    /// Anything else: search, audit, login, and so on.
    Other,
}

/// Work out which package, if any, a request path refers to.
pub fn parse_route(path: &str) -> Route {
    let path = path.split('?').next().unwrap_or(path);
    let path = path.trim_start_matches('/');
    if path.is_empty() {
        return Route::Other;
    }

    // Scoped names are often percent-encoded as `@scope%2fname`.
    let decoded = path.replace("%2F", "/").replace("%2f", "/");

    if let Some((name, _file)) = decoded.split_once("/-/") {
        return match valid_name(name) {
            true => Route::Tarball(name.to_string()),
            false => Route::Other,
        };
    }

    let segments: Vec<&str> = decoded.split('/').collect();
    let (name, rest) = if segments[0].starts_with('@') {
        if segments.len() < 2 {
            return Route::Other;
        }
        (format!("{}/{}", segments[0], segments[1]), &segments[2..])
    } else {
        (segments[0].to_string(), &segments[1..])
    };

    if !valid_name(&name) {
        return Route::Other;
    }

    match rest.len() {
        0 => Route::Packument(name),
        1 => Route::Manifest(name),
        _ => Route::Other,
    }
}

/// Reject registry endpoints that would otherwise parse as package names.
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && !matches!(
            name,
            "-" | "_" | "npm" | "search" | "well-known" | ".well-known"
        )
}

/// `sha512-<base64>`, the integrity format registries publish and clients check.
pub fn integrity_of(tarball: &[u8]) -> String {
    use base64::Engine;
    use sha2::Digest;

    let digest = sha2::Sha512::digest(tarball);
    format!(
        "sha512-{}",
        base64::engine::general_purpose::STANDARD.encode(digest)
    )
}

/// Hex sha1, still read by older clients when integrity is absent.
pub fn shasum_of(tarball: &[u8]) -> String {
    use sha1::Digest;

    sha1::Sha1::digest(tarball)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Point every version in a packument at our tarball's hashes, so whichever
/// version the client resolves, the bytes we serve pass its integrity check.
pub fn rewrite_packument(body: &[u8], tarball: &[u8]) -> Result<Vec<u8>, String> {
    let mut doc: Value =
        serde_json::from_slice(body).map_err(|e| format!("packument is not JSON: {e}"))?;

    let integrity = integrity_of(tarball);
    let shasum = shasum_of(tarball);

    match doc.get_mut("versions").and_then(Value::as_object_mut) {
        Some(versions) => {
            for version in versions.values_mut() {
                rewrite_dist(version, &integrity, &shasum, tarball.len());
            }
        }
        None => return Err("packument has no versions".into()),
    }

    serde_json::to_vec(&doc).map_err(|e| format!("could not re-encode packument: {e}"))
}

/// The same rewrite for a single-version manifest.
pub fn rewrite_manifest(body: &[u8], tarball: &[u8]) -> Result<Vec<u8>, String> {
    let mut doc: Value =
        serde_json::from_slice(body).map_err(|e| format!("manifest is not JSON: {e}"))?;

    rewrite_dist(
        &mut doc,
        &integrity_of(tarball),
        &shasum_of(tarball),
        tarball.len(),
    );

    serde_json::to_vec(&doc).map_err(|e| format!("could not re-encode manifest: {e}"))
}

fn rewrite_dist(version: &mut Value, integrity: &str, shasum: &str, size: usize) {
    let Some(dist) = version.get_mut("dist").and_then(Value::as_object_mut) else {
        return;
    };

    dist.insert("integrity".into(), Value::String(integrity.to_string()));
    dist.insert("shasum".into(), Value::String(shasum.to_string()));
    dist.insert("unpackedSize".into(), Value::from(size));

    // Registry signatures are checked against npm's public key and cannot be
    // reproduced for bytes we packed ourselves. Leaving them would make the
    // client reject the tarball outright.
    dist.remove("signatures");
    dist.remove("npm-signature");
    dist.remove("fileCount");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_plain_packument() {
        assert_eq!(
            parse_route("/is-number"),
            Route::Packument("is-number".into())
        );
    }

    #[test]
    fn parses_a_scoped_packument() {
        assert_eq!(
            parse_route("/@scope/pkg"),
            Route::Packument("@scope/pkg".into())
        );
    }

    #[test]
    fn parses_a_percent_encoded_scope() {
        assert_eq!(
            parse_route("/@scope%2Fpkg"),
            Route::Packument("@scope/pkg".into())
        );
    }

    #[test]
    fn parses_a_version_manifest() {
        assert_eq!(
            parse_route("/is-number/7.0.0"),
            Route::Manifest("is-number".into())
        );
        assert_eq!(
            parse_route("/@scope/pkg/1.0.0"),
            Route::Manifest("@scope/pkg".into())
        );
    }

    #[test]
    fn parses_tarballs_for_both_name_shapes() {
        assert_eq!(
            parse_route("/is-number/-/is-number-7.0.0.tgz"),
            Route::Tarball("is-number".into())
        );
        assert_eq!(
            parse_route("/@scope/pkg/-/pkg-1.0.0.tgz"),
            Route::Tarball("@scope/pkg".into())
        );
    }

    #[test]
    fn ignores_query_strings() {
        assert_eq!(
            parse_route("/is-number?write=true"),
            Route::Packument("is-number".into())
        );
    }

    #[test]
    fn treats_registry_endpoints_as_other() {
        assert_eq!(parse_route("/-/v1/search?text=vue"), Route::Other);
        assert_eq!(parse_route("/-/npm/v1/security/audits"), Route::Other);
        assert_eq!(parse_route("/"), Route::Other);
    }

    #[test]
    fn integrity_is_the_published_format() {
        let integrity = integrity_of(b"hello");
        assert!(integrity.starts_with("sha512-"));
        // sha512 is 64 bytes, which base64-encodes to 88 characters.
        assert_eq!(integrity.len(), "sha512-".len() + 88);
    }

    #[test]
    fn shasum_is_hex_sha1() {
        assert_eq!(
            shasum_of(b"hello"),
            "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d"
        );
    }

    #[test]
    fn every_version_gets_our_hashes() {
        let upstream = br#"{"name":"x","versions":{
            "1.0.0":{"dist":{"tarball":"https://r/x-1.0.0.tgz","integrity":"sha512-old","signatures":[{"sig":"s"}]}},
            "2.0.0":{"dist":{"tarball":"https://r/x-2.0.0.tgz","integrity":"sha512-older"}}
        }}"#;

        let out = rewrite_packument(upstream, b"tarball bytes").unwrap();
        let doc: Value = serde_json::from_slice(&out).unwrap();
        let expected = integrity_of(b"tarball bytes");

        for version in ["1.0.0", "2.0.0"] {
            let dist = &doc["versions"][version]["dist"];
            assert_eq!(dist["integrity"], expected);
            assert!(dist.get("signatures").is_none());
            // The tarball URL is left alone: we intercept the host it points at.
            assert!(dist["tarball"].as_str().unwrap().contains(version));
        }
    }

    #[test]
    fn manifest_rewrite_matches_packument_rewrite() {
        let manifest = br#"{"name":"x","version":"1.0.0","dist":{"integrity":"sha512-old"}}"#;
        let out = rewrite_manifest(manifest, b"bytes").unwrap();
        let doc: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(doc["dist"]["integrity"], integrity_of(b"bytes"));
    }

    #[test]
    fn a_packument_without_versions_is_an_error() {
        assert!(rewrite_packument(br#"{"name":"x"}"#, b"bytes").is_err());
    }
}
