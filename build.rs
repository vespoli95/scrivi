fn main() {
    println!("cargo:rerun-if-env-changed=SCRIVI_VERSION");
    println!("cargo:rerun-if-env-changed=SCRIVI_REPOSITORY");
    println!("cargo:rerun-if-changed=assets/icon.ico");

    if cfg!(windows) && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        if let Ok(version) = std::env::var("SCRIVI_VERSION") {
            if let Some(packed) = pack_windows_version(&version) {
                res.set_version_info(winresource::VersionInfo::FILEVERSION, packed);
                res.set_version_info(winresource::VersionInfo::PRODUCTVERSION, packed);
            }
        }
        res.compile().expect("failed to compile Windows icon resource");
    }
}

fn pack_windows_version(version: &str) -> Option<u64> {
    let stripped = version.strip_prefix('v')?;
    let mut parts = stripped.split('.');
    let major: u64 = parts.next()?.parse().ok()?;
    let minor: u64 = parts.next()?.parse().ok()?;
    let patch: u64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || major > 65535 || minor > 65535 || patch > 65535 {
        return None;
    }
    Some((major << 48) | (minor << 32) | (patch << 16))
}
