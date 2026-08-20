use rcgen::{
    BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair,
    KeyUsagePurpose,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use super::smuggle_home;

/// How long generated leaf certificates stay valid. Kept under Apple's 398-day
/// server-auth limit so the same CA works for clients using the system trust
/// store, not just Node via NODE_EXTRA_CA_CERTS.
const LEAF_VALIDITY_DAYS: i64 = 90;
const CA_VALIDITY_DAYS: i64 = 3650;

pub fn ca_dir() -> PathBuf {
    smuggle_home().join("ca")
}

pub fn cert_path() -> PathBuf {
    ca_dir().join("ca.pem")
}

pub fn key_path() -> PathBuf {
    ca_dir().join("ca.key")
}

pub fn exists() -> bool {
    cert_path().exists() && key_path().exists()
}

/// Generate the smuggle root CA and write it to `~/.smuggle/ca/`.
/// Returns an error if one already exists, so callers must decide about reuse.
pub fn create() -> Result<(), String> {
    if exists() {
        return Err("a smuggle CA already exists".into());
    }

    let key = KeyPair::generate().map_err(|e| format!("failed to generate CA key: {e}"))?;

    let mut params =
        CertificateParams::new(Vec::<String>::new()).map_err(|e| format!("bad CA params: {e}"))?;
    params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
        KeyUsagePurpose::DigitalSignature,
    ];
    params
        .distinguished_name
        .push(DnType::CommonName, "smuggle local development CA");
    params
        .distinguished_name
        .push(DnType::OrganizationName, "smuggle");
    params.not_before = now();
    params.not_after = now() + time::Duration::days(CA_VALIDITY_DAYS);

    let cert = params
        .self_signed(&key)
        .map_err(|e| format!("failed to self-sign CA: {e}"))?;

    let dir = ca_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("failed to create {}: {e}", dir.display()))?;
    write_private(&key_path(), key.serialize_pem().as_bytes())?;
    std::fs::write(cert_path(), cert.pem()).map_err(|e| format!("failed to write CA cert: {e}"))?;

    Ok(())
}

/// Remove the CA material from disk. Not an error if it was never there.
pub fn delete() -> Result<(), String> {
    let dir = ca_dir();
    if !dir.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(&dir).map_err(|e| format!("failed to remove {}: {e}", dir.display()))
}

fn now() -> time::OffsetDateTime {
    time::OffsetDateTime::now_utc()
}

fn write_private(path: &std::path::Path, contents: &[u8]) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| format!("failed to open {}: {e}", path.display()))?;
    file.write_all(contents)
        .map_err(|e| format!("failed to write {}: {e}", path.display()))
}

/// Signs per-host leaf certificates on demand, caching them for the process
/// lifetime so repeated connections to the same registry reuse one cert.
pub struct LeafSigner {
    issuer: Issuer<'static, KeyPair>,
    cache: Mutex<HashMap<String, Leaf>>,
}

#[derive(Clone)]
pub struct Leaf {
    pub cert_pem: String,
    pub key_pem: String,
}

impl LeafSigner {
    pub fn load() -> Result<Self, String> {
        let cert_pem = std::fs::read_to_string(cert_path())
            .map_err(|e| format!("failed to read CA cert ({e}) — run `smuggle setup` first"))?;
        let key_pem = std::fs::read_to_string(key_path())
            .map_err(|e| format!("failed to read CA key ({e}) — run `smuggle setup` first"))?;

        let key = KeyPair::from_pem(&key_pem).map_err(|e| format!("bad CA key: {e}"))?;
        let issuer =
            Issuer::from_ca_cert_pem(&cert_pem, key).map_err(|e| format!("bad CA cert: {e}"))?;

        Ok(Self {
            issuer,
            cache: Mutex::new(HashMap::new()),
        })
    }

    pub fn leaf_for(&self, host: &str) -> Result<Leaf, String> {
        if let Some(hit) = self.cache.lock().unwrap().get(host) {
            return Ok(hit.clone());
        }

        let key = KeyPair::generate().map_err(|e| format!("failed to generate leaf key: {e}"))?;

        let mut params = CertificateParams::new(vec![host.to_string()])
            .map_err(|e| format!("bad SAN {host}: {e}"))?;
        params.distinguished_name.push(DnType::CommonName, host);
        params.is_ca = IsCa::ExplicitNoCa;
        params.use_authority_key_identifier_extension = true;
        params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyEncipherment,
        ];
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        params.not_before = now() - time::Duration::hours(1);
        params.not_after = now() + time::Duration::days(LEAF_VALIDITY_DAYS);

        let cert = params
            .signed_by(&key, &self.issuer)
            .map_err(|e| format!("failed to sign leaf for {host}: {e}"))?;

        let leaf = Leaf {
            cert_pem: cert.pem(),
            key_pem: key.serialize_pem(),
        };
        self.cache
            .lock()
            .unwrap()
            .insert(host.to_string(), leaf.clone());
        Ok(leaf)
    }
}
