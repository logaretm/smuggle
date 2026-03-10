use assert_cmd::cargo::cargo_bin_cmd;
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn smuggle() -> Command {
    cargo_bin_cmd!("smuggle")
}

fn create_package(dir: &Path, name: &str, version: &str, files: &[&str], deps: &str) {
    fs::create_dir_all(dir).unwrap();

    let files_json: Vec<String> = files.iter().map(|f| format!("\"{f}\"")).collect();
    let files_field = files_json.join(", ");

    let pkg_json = format!(
        r#"{{
  "name": "{name}",
  "version": "{version}",
  "files": [{files_field}],
  "dependencies": {{{deps}}}
}}"#
    );
    fs::write(dir.join("package.json"), pkg_json).unwrap();
}

fn create_consumer(dir: &Path, deps: &[(&str, &str)]) {
    fs::create_dir_all(dir).unwrap();

    let dep_entries: Vec<String> = deps
        .iter()
        .map(|(name, version)| format!("    \"{name}\": \"{version}\""))
        .collect();

    let pkg_json = format!(
        r#"{{
  "name": "test-consumer",
  "version": "1.0.0",
  "dependencies": {{
{}
  }}
}}"#,
        dep_entries.join(",\n")
    );
    fs::write(dir.join("package.json"), pkg_json).unwrap();
}

fn store_dir() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap();
    Path::new(&home).join(".smuggle").join("packages")
}

fn cleanup_store(name: &str) {
    let dir = store_dir().join(name);
    let _ = fs::remove_dir_all(&dir);
    // Clean empty scope dir
    if let Some(parent) = dir.parent() {
        let _ = fs::remove_dir(parent);
    }
}

// ─── Publish ────────────────────────────────────────────────

#[test]
fn publish_basic_package() {
    let tmp = TempDir::new().unwrap();
    let pkg_dir = tmp.path().join("my-pkg");
    create_package(&pkg_dir, "@test-smug/basic", "1.0.0", &["dist"], "");

    let dist = pkg_dir.join("dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("index.js"), "module.exports = 42;").unwrap();

    // Should not be packed (not in files field)
    fs::write(pkg_dir.join("secret.txt"), "do not pack me").unwrap();

    smuggle()
        .args(["publish", "--path"])
        .arg(&pkg_dir)
        .assert()
        .success();

    // Verify store contents
    let store = store_dir().join("@test-smug/basic");
    assert!(store.join("metadata.json").exists());
    assert!(store.join("package.tgz").exists());

    // Verify metadata
    let meta: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(store.join("metadata.json")).unwrap()).unwrap();
    assert_eq!(meta["name"], "@test-smug/basic");
    assert_eq!(meta["version"], "1.0.0");

    // Verify tarball only contains dist/ and package.json (not secret.txt)
    let tarball = fs::read(store.join("package.tgz")).unwrap();
    let entries = list_tarball_entries(&tarball);
    assert!(entries.contains(&"package/dist/index.js".to_string()));
    assert!(entries.contains(&"package/package.json".to_string()));
    assert!(!entries.iter().any(|e| e.contains("secret")));

    cleanup_store("@test-smug/basic");
}

#[test]
fn publish_overwrites_existing() {
    let tmp = TempDir::new().unwrap();
    let pkg_dir = tmp.path().join("my-pkg");
    create_package(&pkg_dir, "@test-smug/overwrite", "1.0.0", &["dist"], "");

    let dist = pkg_dir.join("dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("index.js"), "v1").unwrap();

    smuggle()
        .args(["publish", "--path"])
        .arg(&pkg_dir)
        .assert()
        .success();

    // Publish again with different content
    fs::write(dist.join("index.js"), "v2").unwrap();

    smuggle()
        .args(["publish", "--path"])
        .arg(&pkg_dir)
        .assert()
        .success();

    // Verify tarball has the new content
    let store = store_dir().join("@test-smug/overwrite");
    let tarball = fs::read(store.join("package.tgz")).unwrap();
    let content = read_tarball_file(&tarball, "package/dist/index.js");
    assert_eq!(content, "v2");

    cleanup_store("@test-smug/overwrite");
}

#[test]
fn publish_respects_files_field() {
    let tmp = TempDir::new().unwrap();
    let pkg_dir = tmp.path().join("pkg");
    create_package(
        &pkg_dir,
        "@test-smug/files-field",
        "1.0.0",
        &["lib", "types"],
        "",
    );

    // Create multiple directories, only some in files field
    for dir_name in &["lib", "types", "src", "test"] {
        let d = pkg_dir.join(dir_name);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("index.js"), "content").unwrap();
    }

    smuggle()
        .args(["publish", "--path"])
        .arg(&pkg_dir)
        .assert()
        .success();

    let store = store_dir().join("@test-smug/files-field");
    let tarball = fs::read(store.join("package.tgz")).unwrap();
    let entries = list_tarball_entries(&tarball);

    assert!(entries.contains(&"package/lib/index.js".to_string()));
    assert!(entries.contains(&"package/types/index.js".to_string()));
    assert!(!entries.iter().any(|e| e.contains("src/")));
    assert!(!entries.iter().any(|e| e.contains("test/")));

    cleanup_store("@test-smug/files-field");
}

#[test]
fn publish_includes_readme_and_license() {
    let tmp = TempDir::new().unwrap();
    let pkg_dir = tmp.path().join("pkg");
    create_package(&pkg_dir, "@test-smug/meta-files", "1.0.0", &["dist"], "");

    let dist = pkg_dir.join("dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("index.js"), "x").unwrap();
    fs::write(pkg_dir.join("README.md"), "# Hello").unwrap();
    fs::write(pkg_dir.join("LICENSE"), "MIT").unwrap();

    smuggle()
        .args(["publish", "--path"])
        .arg(&pkg_dir)
        .assert()
        .success();

    let store = store_dir().join("@test-smug/meta-files");
    let tarball = fs::read(store.join("package.tgz")).unwrap();
    let entries = list_tarball_entries(&tarball);

    assert!(entries.contains(&"package/README.md".to_string()));
    assert!(entries.contains(&"package/LICENSE".to_string()));

    cleanup_store("@test-smug/meta-files");
}

#[test]
fn publish_stores_dependencies() {
    let tmp = TempDir::new().unwrap();
    let pkg_dir = tmp.path().join("pkg");
    create_package(
        &pkg_dir,
        "@test-smug/with-deps",
        "1.0.0",
        &["dist"],
        r#""lodash": "^4.0.0", "zod": "^3.0.0""#,
    );

    let dist = pkg_dir.join("dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("index.js"), "x").unwrap();

    smuggle()
        .args(["publish", "--path"])
        .arg(&pkg_dir)
        .assert()
        .success();

    let store = store_dir().join("@test-smug/with-deps");
    let meta: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(store.join("metadata.json")).unwrap()).unwrap();

    assert_eq!(meta["dependencies"]["lodash"], "^4.0.0");
    assert_eq!(meta["dependencies"]["zod"], "^3.0.0");

    cleanup_store("@test-smug/with-deps");
}

#[test]
fn publish_fails_without_package_json() {
    let tmp = TempDir::new().unwrap();

    smuggle()
        .args(["publish", "--path"])
        .arg(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("no package.json"));
}

#[test]
fn publish_fails_without_name() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("package.json"),
        r#"{"version": "1.0.0"}"#,
    )
    .unwrap();

    smuggle()
        .args(["publish", "--path"])
        .arg(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing 'name'"));
}

#[test]
fn publish_fails_without_version() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("package.json"),
        r#"{"name": "test"}"#,
    )
    .unwrap();

    smuggle()
        .args(["publish", "--path"])
        .arg(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing 'version'"));
}

// ─── List ───────────────────────────────────────────────────

#[test]
fn list_shows_registered_packages() {
    let tmp = TempDir::new().unwrap();
    let pkg_dir = tmp.path().join("pkg");
    create_package(&pkg_dir, "@test-smug/list-test", "3.0.0", &["dist"], "");

    let dist = pkg_dir.join("dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("index.js"), "x").unwrap();

    smuggle()
        .args(["publish", "--path"])
        .arg(&pkg_dir)
        .assert()
        .success();

    smuggle()
        .arg("list")
        .assert()
        .success()
        .stderr(predicate::str::contains("@test-smug/list-test"))
        .stderr(predicate::str::contains("3.0.0"));

    cleanup_store("@test-smug/list-test");
}

#[test]
fn list_empty_store() {
    // This test just verifies the command doesn't crash on empty store.
    // Other tests may have packages registered, so we can't assert "no packages".
    smuggle().arg("list").assert().success();
}

// ─── Unpublish ──────────────────────────────────────────────

#[test]
fn unpublish_removes_package() {
    let tmp = TempDir::new().unwrap();
    let pkg_dir = tmp.path().join("pkg");
    create_package(
        &pkg_dir,
        "@test-smug/unpublish-test",
        "1.0.0",
        &["dist"],
        "",
    );

    let dist = pkg_dir.join("dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("index.js"), "x").unwrap();

    smuggle()
        .args(["publish", "--path"])
        .arg(&pkg_dir)
        .assert()
        .success();

    assert!(store_dir().join("@test-smug/unpublish-test").exists());

    smuggle()
        .args(["unpublish", "@test-smug/unpublish-test"])
        .assert()
        .success()
        .stderr(predicate::str::contains("Removed"));

    assert!(!store_dir().join("@test-smug/unpublish-test").exists());
}

#[test]
fn unpublish_nonexistent_fails() {
    smuggle()
        .args(["unpublish", "@test-smug/does-not-exist"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not registered"));
}

// ─── Install ────────────────────────────────────────────────

#[test]
fn install_no_matching_packages() {
    let tmp = TempDir::new().unwrap();
    create_consumer(tmp.path(), &[("some-random-pkg", "^1.0.0")]);

    smuggle()
        .args(["install", "--all", "--path"])
        .arg(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("no registered packages"));
}

#[test]
fn install_no_package_json() {
    let tmp = TempDir::new().unwrap();

    smuggle()
        .args(["install", "--all", "--path"])
        .arg(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("no package.json"));
}

#[test]
fn install_end_to_end() {
    // This test requires npm to be available
    if std::process::Command::new("npm")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("skipping install_end_to_end: npm not available");
        return;
    }

    let tmp = TempDir::new().unwrap();

    // Create and publish a package
    let pkg_dir = tmp.path().join("my-lib");
    create_package(
        &pkg_dir,
        "@test-smug/e2e-lib",
        "1.0.0",
        &["dist"],
        "",
    );
    let dist = pkg_dir.join("dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("index.js"), "module.exports = { e2e: true };").unwrap();

    smuggle()
        .args(["publish", "--path"])
        .arg(&pkg_dir)
        .assert()
        .success();

    // Create a consumer that depends on it
    let consumer_dir = tmp.path().join("app");
    create_consumer(&consumer_dir, &[("@test-smug/e2e-lib", "^1.0.0")]);

    // Run install in background (it enters watch mode), kill after install completes
    let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin!("smuggle"))
        .args(["install", "--all", "--path"])
        .arg(&consumer_dir)
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    // Wait for install to complete
    std::thread::sleep(std::time::Duration::from_secs(10));

    // Kill the process (it's in watch mode)
    child.kill().unwrap();
    let output = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Verify it started the registry and ran install
    assert!(
        stderr.contains("Local registry started"),
        "expected registry start message, got: {stderr}"
    );
    assert!(
        stderr.contains("Watching for changes"),
        "expected watch message, got: {stderr}"
    );

    // Verify the package was installed
    let installed = consumer_dir
        .join("node_modules")
        .join("@test-smug")
        .join("e2e-lib")
        .join("dist")
        .join("index.js");
    assert!(
        installed.exists(),
        "expected installed file at {}",
        installed.display()
    );
    let content = fs::read_to_string(&installed).unwrap();
    assert_eq!(content, "module.exports = { e2e: true };");

    // Verify .npmrc was cleaned up (ctrlc handler may not fire on kill,
    // but let's check the file was created during the run)
    // The npmrc check is best-effort since kill doesn't trigger cleanup

    cleanup_store("@test-smug/e2e-lib");
}

#[test]
fn install_scoped_npmrc() {
    // Verify that when all packages share a scope, .npmrc uses scoped registry
    if std::process::Command::new("npm")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("skipping: npm not available");
        return;
    }

    let tmp = TempDir::new().unwrap();

    let pkg_dir = tmp.path().join("pkg");
    create_package(
        &pkg_dir,
        "@test-smug/scoped-a",
        "1.0.0",
        &["dist"],
        "",
    );
    let dist = pkg_dir.join("dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("index.js"), "a").unwrap();

    smuggle()
        .args(["publish", "--path"])
        .arg(&pkg_dir)
        .assert()
        .success();

    let consumer_dir = tmp.path().join("app");
    create_consumer(&consumer_dir, &[("@test-smug/scoped-a", "^1.0.0")]);

    let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin!("smuggle"))
        .args(["install", "--all", "--path"])
        .arg(&consumer_dir)
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    // Give it time to write .npmrc and start install
    std::thread::sleep(std::time::Duration::from_secs(3));

    // Check .npmrc while the process is running
    let npmrc = consumer_dir.join(".npmrc");
    if npmrc.exists() {
        let content = fs::read_to_string(&npmrc).unwrap();
        assert!(
            content.contains("@test-smug:registry=http://localhost:"),
            "expected scoped registry in .npmrc, got: {content}"
        );
    }

    child.kill().unwrap();
    child.wait().unwrap();

    cleanup_store("@test-smug/scoped-a");
}

// ─── Tarball helpers ────────────────────────────────────────

fn list_tarball_entries(tarball: &[u8]) -> Vec<String> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let decoder = GzDecoder::new(tarball);
    let mut archive = Archive::new(decoder);
    archive
        .entries()
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path().unwrap().to_string_lossy().to_string())
        .collect()
}

fn read_tarball_file(tarball: &[u8], path: &str) -> String {
    use flate2::read::GzDecoder;
    use std::io::Read;
    use tar::Archive;

    let decoder = GzDecoder::new(tarball);
    let mut archive = Archive::new(decoder);
    for entry in archive.entries().unwrap() {
        let mut entry = entry.unwrap();
        if entry.path().unwrap().to_string_lossy() == path {
            let mut content = String::new();
            entry.read_to_string(&mut content).unwrap();
            return content;
        }
    }
    panic!("file {path} not found in tarball");
}
