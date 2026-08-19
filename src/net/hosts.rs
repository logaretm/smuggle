//! Management of the `/etc/hosts` block that redirects registry traffic at the
//! daemon. The block exists only while the daemon is alive: it is written on
//! start and removed on every exit path. Because a SIGKILL cannot run a
//! handler, every smuggle invocation calls [`reconcile`] first to strip a block
//! whose owning process is gone.

use std::path::Path;

const HOSTS_PATH: &str = "/etc/hosts";
const BEGIN: &str = "# smuggle:begin";
const END: &str = "# smuggle:end";

pub struct Block {
    pub pid: i32,
    pub hosts: Vec<String>,
}

/// Parse the smuggle block out of `/etc/hosts`, if present.
pub fn read_block() -> Option<Block> {
    let contents = std::fs::read_to_string(HOSTS_PATH).ok()?;
    parse_block(&contents)
}

fn parse_block(contents: &str) -> Option<Block> {
    let mut lines = contents.lines();
    let begin = lines.find(|l| l.trim_start().starts_with(BEGIN))?;

    let pid = begin
        .split_whitespace()
        .find_map(|tok| tok.strip_prefix("pid="))
        .and_then(|v| v.parse::<i32>().ok())?;

    let hosts = lines
        .take_while(|l| !l.trim_start().starts_with(END))
        .filter_map(|l| l.split_whitespace().nth(1).map(str::to_string))
        .collect();

    Some(Block { pid, hosts })
}

/// True when a process with this pid is running.
pub fn pid_alive(pid: i32) -> bool {
    pid > 0 && unsafe { libc::kill(pid, 0) } == 0
}

/// Point `hosts` at the daemon's listen address, tagging the block with `pid`.
/// Replaces any existing block. Requires root.
pub fn install(pid: i32, listen_ip: &str, hosts: &[String]) -> Result<(), String> {
    let contents = std::fs::read_to_string(HOSTS_PATH)
        .map_err(|e| format!("failed to read {HOSTS_PATH}: {e}"))?;

    let mut out = strip_block(&contents);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&format!("{BEGIN} pid={pid}\n"));
    for host in hosts {
        out.push_str(&format!("{listen_ip}\t{host}\n"));
    }
    out.push_str(&format!("{END}\n"));

    write_hosts(&out)?;
    flush_dns_cache();
    Ok(())
}

/// Remove the smuggle block. Not an error if there isn't one. Requires root.
pub fn remove() -> Result<(), String> {
    let contents = std::fs::read_to_string(HOSTS_PATH)
        .map_err(|e| format!("failed to read {HOSTS_PATH}: {e}"))?;

    if !contents.contains(BEGIN) {
        return Ok(());
    }

    write_hosts(&strip_block(&contents))?;
    flush_dns_cache();
    Ok(())
}

/// Drop everything between the markers, inclusive.
fn strip_block(contents: &str) -> String {
    let mut out = String::with_capacity(contents.len());
    let mut inside = false;

    for line in contents.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with(BEGIN) {
            inside = true;
            continue;
        }
        if inside {
            if trimmed.starts_with(END) {
                inside = false;
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }

    out
}

/// Write `/etc/hosts` atomically via a sibling temp file so a crash mid-write
/// can never leave the machine without a hosts file.
fn write_hosts(contents: &str) -> Result<(), String> {
    let tmp = Path::new("/etc/.hosts.smuggle.tmp");
    std::fs::write(tmp, contents).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            format!("need root to write {HOSTS_PATH}")
        } else {
            format!("failed to write {}: {e}", tmp.display())
        }
    })?;
    std::fs::rename(tmp, HOSTS_PATH).map_err(|e| {
        let _ = std::fs::remove_file(tmp);
        format!("failed to replace {HOSTS_PATH}: {e}")
    })
}

/// macOS resolves through mDNSResponder, which caches independently of the
/// hosts file. Without this, edits take effect only after its TTL expires.
fn flush_dns_cache() {
    let _ = std::process::Command::new("dscacheutil")
        .arg("-flushcache")
        .status();
    let _ = std::process::Command::new("killall")
        .args(["-HUP", "mDNSResponder"])
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAIN: &str = "127.0.0.1\tlocalhost\n::1\tlocalhost\n";

    fn with_block(pid: i32) -> String {
        format!(
            "{PLAIN}{BEGIN} pid={pid}\n127.0.0.2\tregistry.npmjs.org\n127.0.0.2\tregistry.yarnpkg.com\n{END}\n"
        )
    }

    #[test]
    fn parses_pid_and_hosts() {
        let block = parse_block(&with_block(4242)).unwrap();
        assert_eq!(block.pid, 4242);
        assert_eq!(block.hosts, ["registry.npmjs.org", "registry.yarnpkg.com"]);
    }

    #[test]
    fn no_block_parses_to_none() {
        assert!(parse_block(PLAIN).is_none());
    }

    #[test]
    fn strip_restores_original() {
        assert_eq!(strip_block(&with_block(1)), PLAIN);
    }

    #[test]
    fn strip_is_a_noop_without_a_block() {
        assert_eq!(strip_block(PLAIN), PLAIN);
    }

    #[test]
    fn strip_keeps_entries_after_the_block() {
        let input = format!("{}10.0.0.1\tinternal.example\n", with_block(7));
        assert_eq!(
            strip_block(&input),
            format!("{PLAIN}10.0.0.1\tinternal.example\n")
        );
    }

    #[test]
    fn pid_zero_is_never_alive() {
        assert!(!pid_alive(0));
    }

    #[test]
    fn our_own_pid_is_alive() {
        assert!(pid_alive(std::process::id() as i32));
    }
}
