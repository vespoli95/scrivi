use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use serde::Deserialize;
use sha2::{Digest, Sha256};

pub const APP_VERSION: &str = match option_env!("SCRIVI_VERSION") {
    Some(v) => v,
    None => "dev",
};

pub const RELEASE_REPOSITORY: &str = match option_env!("SCRIVI_REPOSITORY") {
    Some(r) => r,
    None => "vespoli95/scrivi",
};

pub const MAX_INSTALLER_SIZE: u64 = 128 << 20;
const MAX_RELEASE_JSON_SIZE: u64 = 2 << 20;
const MAX_CHECKSUM_SIZE: u64 = 1024;

#[derive(Clone, Default)]
pub struct CancelSignal {
    inner: Arc<(Mutex<bool>, Condvar)>,
}

impl CancelSignal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        let (lock, cvar) = &*self.inner;
        let mut canceled = lock.lock().unwrap_or_else(|e| e.into_inner());
        *canceled = true;
        cvar.notify_all();
    }

    pub fn is_canceled(&self) -> bool {
        let (lock, _) = &*self.inner;
        *lock.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Waits up to `duration`, returning `true` if canceled or `false` on timeout.
    pub fn wait_timeout(&self, duration: Duration) -> bool {
        let (lock, cvar) = &*self.inner;
        let mut canceled = lock.lock().unwrap_or_else(|e| e.into_inner());
        if *canceled {
            return true;
        }
        let (guard, _) = cvar
            .wait_timeout_while(canceled, duration, |c| !*c)
            .unwrap_or_else(|e| e.into_inner());
        canceled = guard;
        *canceled
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateInfo {
    pub version: String,
    pub download_url: String,
}

#[derive(Debug, Deserialize)]
struct ReleaseAsset {
    name: String,
}

#[derive(Debug, Deserialize)]
struct ReleaseMetadata {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<ReleaseAsset>,
}

pub fn parse_version(value: &str) -> Result<[u16; 3], String> {
    let Some(rest) = value.strip_prefix('v') else {
        return Err(format!("invalid release version {value:?}"));
    };
    let parts: Vec<&str> = rest.split('.').collect();
    if parts.len() != 3 {
        return Err(format!("invalid release version {value:?}"));
    }
    let mut parsed = [0u16; 3];
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty()
            || !part.bytes().all(|b| b.is_ascii_digit())
            || (part.len() > 1 && part.starts_with('0'))
        {
            return Err(format!("invalid release version {value:?}"));
        }
        let n: u32 = part
            .parse()
            .map_err(|_| format!("invalid release version {value:?}"))?;
        if n > 65535 {
            return Err(format!("invalid release version {value:?}"));
        }
        parsed[i] = n as u16;
    }
    Ok(parsed)
}

pub fn newer_version(current: &str, latest: &str) -> Result<bool, String> {
    let old_version = parse_version(current)?;
    let new_version = parse_version(latest)?;
    Ok(new_version > old_version)
}

/// Returns a fixed GitHub release download URL, never trusting URLs inside release metadata.
pub fn find_update(
    agent: &ureq::Agent,
    endpoint: &str,
    repository: &str,
    current: &str,
) -> Result<Option<UpdateInfo>, String> {
    let response = match agent
        .get(endpoint)
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", &format!("scrivi/{current}"))
        .call()
    {
        Ok(resp) => resp,
        Err(ureq::Error::Status(404, _)) => return Ok(None),
        Err(ureq::Error::Status(code, _)) => {
            return Err(format!("check for updates: GitHub returned HTTP {code}"));
        }
        Err(err) => return Err(format!("check for updates: {err}")),
    };

    let mut body = Vec::new();
    response
        .into_reader()
        .take(MAX_RELEASE_JSON_SIZE + 1)
        .read_to_end(&mut body)
        .map_err(|e| format!("read update information: {e}"))?;
    if body.len() as u64 > MAX_RELEASE_JSON_SIZE {
        return Err("read update information: response exceeds limit".into());
    }

    let release: ReleaseMetadata =
        serde_json::from_slice(&body).map_err(|e| format!("read update information: {e}"))?;

    if release.draft || release.prerelease {
        return Ok(None);
    }
    if !newer_version(current, &release.tag_name)? {
        return Ok(None);
    }

    let filename = format!("scrivi-{}-windows-x64-setup.exe", release.tag_name);
    if release.assets.iter().any(|asset| asset.name == filename) {
        return Ok(Some(UpdateInfo {
            download_url: format!(
                "https://github.com/{repository}/releases/download/{}/{filename}",
                release.tag_name
            ),
            version: release.tag_name,
        }));
    }

    Err(format!(
        "release {} has no Windows installer",
        release.tag_name
    ))
}

fn update_response(
    cancel: &CancelSignal,
    agent: &ureq::Agent,
    address: &str,
) -> Result<ureq::Response, String> {
    if cancel.is_canceled() {
        return Err("download update: canceled".into());
    }
    match agent.get(address).call() {
        Ok(resp) => {
            if resp.status() != 200 {
                return Err(format!("download update: HTTP {}", resp.status()));
            }
            Ok(resp)
        }
        Err(ureq::Error::Status(code, _)) => Err(format!("download update: HTTP {code}")),
        Err(err) => Err(format!("download update: {err}")),
    }
}

fn decode_sha256_hex(text: &str) -> Result<[u8; 32], String> {
    let trimmed = text.trim();
    if trimmed.len() != 64 || !trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("invalid installer checksum".into());
    }
    let mut out = [0u8; 32];
    let bytes = trimmed.as_bytes();
    for (i, slot) in out.iter_mut().enumerate() {
        let hex_pair = std::str::from_utf8(&bytes[i * 2..i * 2 + 2])
            .map_err(|_| "invalid installer checksum")?;
        *slot = u8::from_str_radix(hex_pair, 16).map_err(|_| "invalid installer checksum")?;
    }
    Ok(out)
}

pub fn download_installer(
    cancel: &CancelSignal,
    agent: &ureq::Agent,
    download: &str,
    dir: &Path,
) -> Result<PathBuf, String> {
    let checksum_resp = update_response(cancel, agent, &format!("{download}.sha256"))?;
    let mut checksum_bytes = Vec::new();
    checksum_resp
        .into_reader()
        .take(MAX_CHECKSUM_SIZE)
        .read_to_end(&mut checksum_bytes)
        .map_err(|e| format!("read installer checksum: {e}"))?;
    let checksum_text =
        std::str::from_utf8(&checksum_bytes).map_err(|_| "invalid installer checksum")?;
    let expected = decode_sha256_hex(checksum_text)?;

    let response = update_response(cancel, agent, download)?;
    let expected_len: Option<u64> = match response.header("Content-Length") {
        Some(raw) => {
            let len: u64 = raw
                .trim()
                .parse()
                .map_err(|_| "download update: invalid Content-Length".to_string())?;
            if len > MAX_INSTALLER_SIZE {
                return Err("installer exceeds the 128 MiB download limit".into());
            }
            Some(len)
        }
        None => None,
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
            .map_err(|e| format!("create update directory: {e}"))?;
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(dir).map_err(|e| format!("create update directory: {e}"))?;
    }

    let mut temp = tempfile::Builder::new()
        .prefix("download-")
        .suffix(".exe")
        .tempfile_in(dir)
        .map_err(|e| format!("create installer download: {e}"))?;

    let mut reader = response.into_reader().take(MAX_INSTALLER_SIZE + 1);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut total: u64 = 0;

    loop {
        if cancel.is_canceled() {
            return Err("download update: canceled".into());
        }
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("download installer: {e}"))?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > MAX_INSTALLER_SIZE {
            return Err("installer exceeds the 128 MiB download limit".into());
        }
        temp.write_all(&buf[..n])
            .map_err(|e| format!("save installer: {e}"))?;
        hasher.update(&buf[..n]);
    }

    temp.flush().map_err(|e| format!("save installer: {e}"))?;

    if let Some(len) = expected_len {
        if total != len {
            return Err("download installer: incomplete download".into());
        }
    }

    let actual_digest: [u8; 32] = hasher.finalize().into();
    if actual_digest != expected {
        return Err("installer checksum does not match; update was not installed".into());
    }

    let path = dir.join("setup.exe");
    temp.persist(&path)
        .map_err(|e| format!("save verified installer: {}", e.error))?;
    Ok(path)
}

pub fn watch_updates(cancel: CancelSignal, interval: Duration, mut check: impl FnMut()) {
    loop {
        if cancel.is_canceled() {
            return;
        }
        check();
        if cancel.wait_timeout(interval) {
            return;
        }
    }
}

pub fn install_update(cancel: &CancelSignal, download: &str) -> Result<(), String> {
    let cache = dirs::cache_dir().ok_or_else(|| "find update cache".to_string())?;
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(300))
        .build();
    let path = download_installer(cancel, &agent, download, &cache.join("scrivi").join("updates"))?;
    if cancel.is_canceled() {
        return Err("download update: canceled".into());
    }
    Command::new(&path)
        .args(["/SILENT", "/NORESTART", "/SP-", "/RESTARTSCRIVI=1"])
        .spawn()
        .map_err(|e| format!("start update installer: {e}"))?;
    Ok(())
}

#[derive(Debug)]
pub enum UpdateEvent {
    Available(UpdateInfo),
    InstallFinished(Result<(), String>),
}

pub struct UpdateController {
    cancel: CancelSignal,
    rx: mpsc::Receiver<UpdateEvent>,
    tx: mpsc::Sender<UpdateEvent>,
    pub declined_version: Option<String>,
}

impl UpdateController {
    pub fn start(ctx: egui::Context) -> Option<Self> {
        if !cfg!(windows) || APP_VERSION == "dev" {
            return None;
        }
        let cancel = CancelSignal::new();
        let (tx, rx) = mpsc::channel();
        let watcher_cancel = cancel.clone();
        let watcher_tx = tx.clone();

        thread::spawn(move || {
            let endpoint =
                format!("https://api.github.com/repos/{RELEASE_REPOSITORY}/releases/latest");
            watch_updates(watcher_cancel, Duration::from_secs(300), || {
                let agent = ureq::AgentBuilder::new()
                    .timeout(Duration::from_secs(15))
                    .build();
                match find_update(&agent, &endpoint, RELEASE_REPOSITORY, APP_VERSION) {
                    Ok(Some(info)) => {
                        let _ = watcher_tx.send(UpdateEvent::Available(info));
                        ctx.request_repaint();
                    }
                    Ok(None) => {}
                    Err(err) => {
                        eprintln!("Automatic update check failed: {err}");
                    }
                }
            });
        });

        Some(Self {
            cancel,
            rx,
            tx,
            declined_version: None,
        })
    }

    pub fn try_recv(&self) -> Option<UpdateEvent> {
        self.rx.try_recv().ok()
    }

    pub fn start_install(&self, ctx: egui::Context, download_url: String) {
        let cancel = self.cancel.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let res = install_update(&cancel, &download_url);
            let _ = tx.send(UpdateEvent::InstallFinished(res));
            ctx.request_repaint();
        });
    }

    pub fn cancel(&self) {
        self.cancel.cancel();
    }
}

impl Drop for UpdateController {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;
    use tempfile::tempdir;

    fn spawn_http_server(
        handler: impl Fn(&str, &str, &[(String, String)]) -> (u16, Vec<(String, String)>, Vec<u8>)
            + Send
            + Sync
            + 'static,
    ) -> (String, CancelSignal, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let stop = CancelSignal::new();
        let stop_thread = stop.clone();
        let handler = Arc::new(handler);

        let handle = thread::spawn(move || {
            while !stop_thread.is_canceled() {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                        let mut reader = BufReader::new(stream.try_clone().unwrap());
                        let mut request_line = String::new();
                        if reader.read_line(&mut request_line).is_err() || request_line.is_empty() {
                            continue;
                        }
                        let mut parts = request_line.split_whitespace();
                        let method = parts.next().unwrap_or("");
                        let path = parts.next().unwrap_or("/");
                        let mut headers = Vec::new();
                        loop {
                            let mut line = String::new();
                            if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
                                break;
                            }
                            if let Some((k, v)) = line.split_once(':') {
                                headers.push((k.trim().to_string(), v.trim().to_string()));
                            }
                        }
                        let (status, resp_headers, body) = handler(method, path, &headers);
                        let reason = match status {
                            200 => "OK",
                            403 => "Forbidden",
                            404 => "Not Found",
                            503 => "Service Unavailable",
                            _ => "Status",
                        };
                        let mut head = format!("HTTP/1.1 {status} {reason}\r\nConnection: close\r\n");
                        let has_cl = resp_headers
                            .iter()
                            .any(|(k, _)| k.eq_ignore_ascii_case("content-length"));
                        for (k, v) in &resp_headers {
                            head.push_str(&format!("{k}: {v}\r\n"));
                        }
                        if !has_cl {
                            head.push_str(&format!("Content-Length: {}\r\n", body.len()));
                        }
                        head.push_str("\r\n");
                        let _ = stream.write_all(head.as_bytes());
                        let _ = stream.write_all(&body);
                        let _ = stream.flush();
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });

        (format!("http://{addr}"), stop, handle)
    }

    #[test]
    fn test_newer_version() {
        for (current, latest, want) in [
            ("v1.0.0", "v1.0.1", true),
            ("v1.9.9", "v1.10.0", true),
            ("v1.9.9", "v2.0.0", true),
            ("v1.1.0", "v1.1.0", false),
            ("v2.0.0", "v1.99.0", false),
        ] {
            let got = newer_version(current, latest).unwrap();
            assert_eq!(got, want, "{current} -> {latest}");
        }

        for invalid in ["dev", "v1.0", "v1.0.1-beta", "v1.01.0", "v65536.0.0"] {
            assert!(
                newer_version("v1.0.0", invalid).is_err(),
                "accepted invalid version {invalid:?}"
            );
        }
    }

    #[test]
    fn test_find_update() {
        let cases = [
            (
                "new release",
                200u16,
                r#"{"tag_name":"v1.1.0","assets":[{"name":"scrivi-v1.1.0-windows-x64-setup.exe","browser_download_url":"https://untrusted.example/setup.exe"}]}"#,
                true,
                false,
            ),
            ("same version", 200, r#"{"tag_name":"v1.0.0"}"#, false, false),
            ("older release", 200, r#"{"tag_name":"v0.9.0"}"#, false, false),
            (
                "prerelease",
                200,
                r#"{"tag_name":"v2.0.0-beta","prerelease":true}"#,
                false,
                false,
            ),
            (
                "draft",
                200,
                r#"{"tag_name":"v2.0.0","draft":true}"#,
                false,
                false,
            ),
            (
                "missing installer",
                200,
                r#"{"tag_name":"v1.1.0","assets":[]}"#,
                false,
                true,
            ),
            (
                "invalid version",
                200,
                r#"{"tag_name":"../bad"}"#,
                false,
                true,
            ),
            ("invalid response", 200, r#"{"#, false, true),
            ("no releases", 404, "", false, false),
            ("rate limited", 403, "", false, true),
        ];

        for (name, status, body, want_url, want_err) in cases {
            let body_bytes = body.as_bytes().to_vec();
            let (base_url, stop, handle) = spawn_http_server(move |_, _, headers| {
                let ua = headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("user-agent"))
                    .map(|(_, v)| v.as_str());
                assert_eq!(ua, Some("scrivi/v1.0.0"));
                (status, vec![], body_bytes.clone())
            });

            let agent = ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(5))
                .build();
            let res = find_update(&agent, &base_url, "vespoli95/scrivi", "v1.0.0");
            stop.cancel();
            let _ = handle.join();

            assert_eq!(res.is_err(), want_err, "{name}: {res:?}");
            let info = res.unwrap_or(None);
            assert_eq!(info.is_some(), want_url, "{name}: {info:?}");
            if want_url {
                let info = info.unwrap();
                assert_eq!(info.version, "v1.1.0");
                assert_eq!(
                    info.download_url,
                    "https://github.com/vespoli95/scrivi/releases/download/v1.1.0/scrivi-v1.1.0-windows-x64-setup.exe"
                );
            }
        }
    }

    #[test]
    fn test_download_installer() {
        let installer = b"test installer contents";
        let valid_checksum = format!("{:x}\n", Sha256::digest(installer));

        struct Case<'a> {
            name: &'a str,
            checksum: String,
            checksum_status: u16,
            installer_status: u16,
            truncated: bool,
            oversized: bool,
            want_err: bool,
        }

        let cases = [
            Case {
                name: "verified",
                checksum: valid_checksum.clone(),
                checksum_status: 200,
                installer_status: 200,
                truncated: false,
                oversized: false,
                want_err: false,
            },
            Case {
                name: "mismatch",
                checksum: "0".repeat(64),
                checksum_status: 200,
                installer_status: 200,
                truncated: false,
                oversized: false,
                want_err: true,
            },
            Case {
                name: "invalid checksum",
                checksum: "not a checksum".into(),
                checksum_status: 200,
                installer_status: 200,
                truncated: false,
                oversized: false,
                want_err: true,
            },
            Case {
                name: "short checksum",
                checksum: "abcd".into(),
                checksum_status: 200,
                installer_status: 200,
                truncated: false,
                oversized: false,
                want_err: true,
            },
            Case {
                name: "missing checksum",
                checksum: String::new(),
                checksum_status: 404,
                installer_status: 200,
                truncated: false,
                oversized: false,
                want_err: true,
            },
            Case {
                name: "download failure",
                checksum: valid_checksum.clone(),
                checksum_status: 200,
                installer_status: 503,
                truncated: false,
                oversized: false,
                want_err: true,
            },
            Case {
                name: "interrupted download",
                checksum: valid_checksum.clone(),
                checksum_status: 200,
                installer_status: 200,
                truncated: true,
                oversized: false,
                want_err: true,
            },
            Case {
                name: "oversized download",
                checksum: valid_checksum,
                checksum_status: 200,
                installer_status: 200,
                truncated: false,
                oversized: true,
                want_err: true,
            },
        ];

        for tc in cases {
            let checksum = tc.checksum.clone();
            let cs = tc.checksum_status;
            let is = tc.installer_status;
            let truncated = tc.truncated;
            let oversized = tc.oversized;

            let (base_url, stop, handle) = spawn_http_server(move |_, path, _| {
                if path == "/setup.exe.sha256" {
                    return (cs, vec![], checksum.as_bytes().to_vec());
                }
                let mut hdrs = Vec::new();
                if truncated {
                    hdrs.push(("Content-Length".into(), "100".into()));
                } else if oversized {
                    hdrs.push((
                        "Content-Length".into(),
                        ((128u64 << 20) + 1).to_string(),
                    ));
                }
                (is, hdrs, installer.to_vec())
            });

            let dir = tempdir().unwrap();
            let path = dir.path().join("setup.exe");
            fs::write(&path, b"previous download").unwrap();

            let cancel = CancelSignal::new();
            let agent = ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(5))
                .build();
            let res = download_installer(&cancel, &agent, &format!("{base_url}/setup.exe"), dir.path());

            stop.cancel();
            let _ = handle.join();

            assert_eq!(res.is_err(), tc.want_err, "{}: {res:?}", tc.name);
            let want_bytes = if tc.want_err {
                b"previous download".as_slice()
            } else {
                assert_eq!(res.unwrap(), path);
                installer.as_slice()
            };
            assert_eq!(fs::read(&path).unwrap(), want_bytes, "{}", tc.name);
            let entries: Vec<_> = fs::read_dir(dir.path()).unwrap().collect();
            assert_eq!(entries.len(), 1, "{}: temp files left behind", tc.name);
        }
    }

    #[test]
    fn test_download_installer_canceled() {
        let cancel = CancelSignal::new();
        cancel.cancel();
        let dir = tempdir().unwrap();
        let agent = ureq::Agent::new();
        assert!(download_installer(&cancel, &agent, "https://github.com/setup.exe", dir.path()).is_err());
    }

    #[test]
    fn test_watch_updates() {
        let cancel = CancelSignal::new();
        let (tx, rx) = mpsc::channel();
        let worker_cancel = cancel.clone();
        let handle = thread::spawn(move || {
            watch_updates(worker_cancel, Duration::from_millis(1), move || {
                let _ = tx.send(());
            });
        });

        for _ in 0..2 {
            rx.recv_timeout(Duration::from_secs(5))
                .expect("update watcher did not check at startup and again while running");
        }
        cancel.cancel();
        handle.join().unwrap();

        watch_updates(cancel, Duration::from_millis(1), || {
            panic!("canceled watcher must not check for updates");
        });
    }
}
