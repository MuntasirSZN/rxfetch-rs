use crate::platform::Platform;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// A single package-source result.
struct PkgSource {
    name: &'static str,
    count: u32,
}

/// Detect every package source that exists on the system and return a
/// human-readable summary string.
pub fn collect(platform: &Platform) -> String {
    let mut sources: Vec<PkgSource> = Vec::new();

    macro_rules! try_source {
        ($name:expr, $func:expr) => {
            if let Some(c) = $func {
                if c > 0 {
                    sources.push(PkgSource {
                        name: $name,
                        count: c,
                    });
                }
            }
        };
    }

    try_source!("pacman", count_pacman());
    try_source!("dpkg", count_dpkg(platform));
    try_source!("rpm", count_rpm());
    try_source!("xbps", count_xbps());
    try_source!("apk", count_apk());
    try_source!("portage", count_portage());
    try_source!("nix", count_nix());
    try_source!("flatpak", count_flatpak());
    try_source!("snap", count_snap());
    try_source!("brew", count_brew());
    try_source!("eopkg", count_eopkg());
    try_source!("pkgng", count_pkgng());

    if sources.is_empty() {
        return "Unknown".into();
    }

    let total: u32 = sources.iter().map(|s| s.count).sum();

    if sources.len() == 1 {
        format!("{} ({})", total, sources[0].name)
    } else {
        let detail: Vec<String> = sources
            .iter()
            .map(|s| format!("{} {}", s.count, s.name))
            .collect();
        format!("{} ({})", total, detail.join(", "))
    }
}

// ─── Pacman (Arch, Manjaro, etc.) ────────────────────────────────────
// Each sub-directory of /var/lib/pacman/local/ (except ALPM_DB_VERSION)
// represents one installed package.

fn count_pacman() -> Option<u32> {
    let dir = Path::new("/var/lib/pacman/local");
    if !dir.exists() {
        return None;
    }
    let c = count_subdirs(dir, &["ALPM_DB_VERSION"]);
    Some(c)
}

// ─── dpkg (Debian, Ubuntu, Termux, etc.) ─────────────────────────────
// Every installed package has exactly one .list file in
// /var/lib/dpkg/info/ (or $PREFIX/var/lib/dpkg/info on Termux).

fn count_dpkg(platform: &Platform) -> Option<u32> {
    let paths: &[&str] = match platform {
        Platform::Android => &[
            // Termux keeps its own dpkg
            "/data/data/com.termux/files/usr/var/lib/dpkg/info",
        ],
        Platform::MacOS => return None,
        _ => &["/var/lib/dpkg/info"],
    };

    // For Android/Termux, also try $PREFIX/var/lib/dpkg/info
    let prefix_path: Option<PathBuf> = if matches!(platform, Platform::Android) {
        env::var("PREFIX")
            .ok()
            .map(|p| PathBuf::from(p).join("var/lib/dpkg/info"))
    } else {
        None
    };

    // Try $PREFIX path first if it exists
    if let Some(ref pp) = prefix_path
        && pp.exists()
        && let Ok(entries) = fs::read_dir(pp)
    {
        let c = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                let file_name = e.file_name();
                let file_name_str = file_name.to_string_lossy();
                file_name_str.ends_with(".list") && !file_name_str.contains(':') // skip :arch dupes
            })
            .count();
        return Some(c as u32);
    }

    for p in paths {
        let dir = Path::new(p);
        if dir.exists() {
            let c = fs::read_dir(dir)
                .ok()?
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let file_name = e.file_name();
                    let file_name_str = file_name.to_string_lossy();
                    file_name_str.ends_with(".list") && !file_name_str.contains(':') // skip :arch dupes
                })
                .count();
            return Some(c as u32);
        }
    }
    None
}

// ─── RPM (Fedora, RHEL, openSUSE, etc.) ─────────────────────────────
// Modern distros use an sqlite rpmdb. Rather than pulling in a sqlite
// crate we count installed-package entries by reading the Berkeley DB
// fallback directories or the Packages index.
//
// Most reliable portable heuristic: each package installs a directory
// under /usr/share/doc, so when we know rpm exists we use that.
// We validate the rpm db directory exists first.

fn count_rpm() -> Option<u32> {
    let rpm_db_exists =
        Path::new("/var/lib/rpm").exists() || Path::new("/usr/lib/sysimage/rpm").exists();
    if !rpm_db_exists {
        return None;
    }
    // Heuristic: /usr/share/doc dirs
    let doc = Path::new("/usr/share/doc");
    if !doc.exists() {
        return None;
    }
    Some(count_subdirs(doc, &[]))
}

// ─── XBPS (Void Linux) ──────────────────────────────────────────────
// The package database lives in a single plist file, but there is also
// one <pkg>.plist per package under /var/db/xbps/.

fn count_xbps() -> Option<u32> {
    let dir = Path::new("/var/db/xbps");
    if !dir.exists() {
        return None;
    }
    let c = fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.ends_with(".plist") && !name.starts_with("pkgdb")
        })
        .count();
    if c > 0 { Some(c as u32) } else { None }
}

// ─── APK (Alpine Linux) ─────────────────────────────────────────────
// /lib/apk/db/installed is a text file where each package entry starts
// with a line beginning with "P:" (the package name).

fn count_apk() -> Option<u32> {
    let db = Path::new("/lib/apk/db/installed");
    if !db.exists() {
        return None;
    }
    let content = fs::read_to_string(db).ok()?;
    let c = content.lines().filter(|l| l.starts_with("P:")).count();
    Some(c as u32)
}

// ─── Portage / Emerge (Gentoo) ───────────────────────────────────────
// /var/db/pkg/<category>/<name-version>/ — two levels of directories.

fn count_portage() -> Option<u32> {
    let root = Path::new("/var/db/pkg");
    if !root.exists() {
        return None;
    }
    let mut count = 0u32;
    for cat in read_subdirs(root) {
        count += count_subdirs(&cat, &[]);
    }
    if count > 0 { Some(count) } else { None }
}

// ─── Nix ─────────────────────────────────────────────────────────────
// User packages are symlinked from ~/.nix-profile/bin/ into
// /nix/store/<hash>-<name>/...  We collect unique store derivation
// hashes to count distinct packages.

fn count_nix() -> Option<u32> {
    if !Path::new("/nix").exists() {
        return None;
    }
    let home = env::var("HOME").ok()?;
    let bin = PathBuf::from(home).join(".nix-profile/bin");
    if !bin.exists() {
        return None;
    }
    let mut store_paths = HashSet::new();
    for entry in fs::read_dir(&bin).ok()?.flatten() {
        if let Ok(target) = fs::read_link(entry.path()) {
            let s = target.to_string_lossy().to_string();
            if let Some(rest) = s.strip_prefix("/nix/store/")
                && let Some(derivation) = rest.split('/').next()
            {
                store_paths.insert(derivation.to_string());
            }
        }
    }
    if store_paths.is_empty() {
        None
    } else {
        Some(store_paths.len() as u32)
    }
}

// ─── Flatpak ─────────────────────────────────────────────────────────
// System-wide: /var/lib/flatpak/app/<app-id>/
// Per-user:    ~/.local/share/flatpak/app/<app-id>/

fn count_flatpak() -> Option<u32> {
    let sys_dir = Path::new("/var/lib/flatpak/app");
    let user_dir = env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".local/share/flatpak/app"));

    let sys_ok = sys_dir.exists();
    let usr_ok = user_dir.as_ref().map(|p| p.exists()).unwrap_or(false);

    if !sys_ok && !usr_ok {
        return None;
    }

    let mut c = 0u32;
    if sys_ok {
        c += count_subdirs(sys_dir, &[]);
    }
    if let Some(ref ud) = user_dir
        && usr_ok
    {
        c += count_subdirs(ud, &[]);
    }
    Some(c)
}

// ─── Snap ────────────────────────────────────────────────────────────
// /snap/<name>/ directories, skipping meta entries.

fn count_snap() -> Option<u32> {
    let snap = Path::new("/snap");
    if !snap.exists() {
        // Alternative location
        let alt = Path::new("/var/lib/snapd/snaps");
        if !alt.exists() {
            return None;
        }
        let c = fs::read_dir(alt)
            .ok()?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".snap"))
            .count();
        return Some(c as u32);
    }
    let skip = ["bin", "README", ".", ".."];
    let c = fs::read_dir(snap)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let n = e.file_name();
            let n = n.to_string_lossy();
            e.file_type().map(|t| t.is_dir()).unwrap_or(false) && !skip.contains(&n.as_ref())
        })
        .count();
    Some(c as u32)
}

// ─── Homebrew ────────────────────────────────────────────────────────
// Counts directories in Cellar/ (formulas) + Caskroom/ (casks) under
// the standard Homebrew prefixes.

fn count_brew() -> Option<u32> {
    let prefixes = ["/usr/local", "/opt/homebrew", "/home/linuxbrew/.linuxbrew"];

    for pfx in &prefixes {
        let cellar = PathBuf::from(pfx).join("Cellar");
        let caskroom = PathBuf::from(pfx).join("Caskroom");

        if cellar.exists() || caskroom.exists() {
            let mut c = 0u32;
            if cellar.exists() {
                c += count_subdirs(&cellar, &[]);
            }
            if caskroom.exists() {
                c += count_subdirs(&caskroom, &[]);
            }
            return Some(c);
        }
    }
    None
}

// ─── eopkg (Solus) ───────────────────────────────────────────────────
// /var/lib/eopkg/package/ contains one directory per installed package.

fn count_eopkg() -> Option<u32> {
    let dir = Path::new("/var/lib/eopkg/package");
    if !dir.exists() {
        return None;
    }
    Some(count_subdirs(dir, &[]))
}

// ─── pkg (FreeBSD) ───────────────────────────────────────────────────
// /var/db/pkg/local.sqlite exists, but we use the directory listing
// at /usr/local/share/licenses/ as a heuristic (one dir per package).
// Alternatively, /var/db/pkg/ itself has package directories on older
// FreeBSD (pre-pkgng).

fn count_pkgng() -> Option<u32> {
    // Modern pkgng with sqlite — count package dirs in /usr/local/share/licenses
    let licenses = Path::new("/usr/local/share/licenses");
    if licenses.exists() {
        let c = count_subdirs(licenses, &[]);
        if c > 0 {
            return Some(c);
        }
    }
    // Older style: /var/db/pkg/<name-version>/ directories
    let legacy = Path::new("/var/db/pkg");
    if legacy.exists() {
        let c = fs::read_dir(legacy)
            .ok()?
            .filter_map(|e| e.ok())
            .filter(|e| {
                let n = e.file_name();
                let n = n.to_string_lossy();
                e.file_type().map(|t| t.is_dir()).unwrap_or(false) && n != "local.sqlite"
            })
            .count();
        if c > 0 {
            return Some(c as u32);
        }
    }
    None
}

// ─── Helpers ─────────────────────────────────────────────────────────

fn count_subdirs(dir: &Path, skip: &[&str]) -> u32 {
    fs::read_dir(dir)
        .into_iter()
        .flatten() // handle Ok(ReadDir) → entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            e.file_type().map(|t| t.is_dir()).unwrap_or(false) && !skip.contains(&name.as_ref())
        })
        .count() as u32
}

fn read_subdirs(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.path())
        .collect()
}
