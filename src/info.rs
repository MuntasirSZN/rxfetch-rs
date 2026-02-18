use crate::packages;
use crate::platform::Platform;
use nix::sys::statvfs::statvfs;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use sysinfo::{Disks, System};

// ─── Kernel release ──────────────────────────────────────────────────

pub fn kernel_release() -> String {
    // sysinfo provides this on all supported systems
    System::kernel_version().unwrap_or_else(|| {
        // Fallback: /proc/version on Linux
        fs::read_to_string("/proc/version")
            .ok()
            .and_then(|v| v.split_whitespace().nth(2).map(|s| s.to_string()))
            .unwrap_or_else(|| "unknown".into())
    })
}

// ─── Architecture ────────────────────────────────────────────────────

pub fn arch() -> &'static str {
    env::consts::ARCH
}

// Get Linux-style architecture name using uname
#[cfg(unix)]
pub fn linux_arch() -> String {
    // Try using uname crate for cross-platform uname support
    if let Ok(info) = uname::uname() {
        return info.machine;
    }

    // Fallback to mapping Rust's arch constants if uname fails
    match env::consts::ARCH {
        "aarch64" => "armv8l".to_string(),
        "x86_64" => "x86_64".to_string(),
        "x86" => "i686".to_string(),
        "arm" => "armv7l".to_string(),
        arch => arch.to_string(),
    }
}

// Fallback for non-Unix platforms (e.g., Windows)
#[cfg(not(unix))]
pub fn linux_arch() -> String {
    // Just use Rust's arch constants
    match env::consts::ARCH {
        "aarch64" => "armv8l".to_string(),
        "x86_64" => "x86_64".to_string(),
        "x86" => "i686".to_string(),
        "arm" => "armv7l".to_string(),
        arch => arch.to_string(),
    }
}

// ─── Distro / OS name ────────────────────────────────────────────────

pub fn distro_name(platform: &Platform) -> String {
    match platform {
        Platform::Android => "Android".into(),
        Platform::MacOS => {
            // sysinfo gives us os name + version
            let name = System::name().unwrap_or_else(|| "macOS".into());
            let ver = System::os_version().unwrap_or_else(|| "unknown".into());
            format!("{} {}", name, ver)
        }
        Platform::FreeBSD | Platform::Linux => {
            // Try PRETTY_NAME from os-release first
            if let Some(pretty) = read_os_release_value("PRETTY_NAME") {
                return pretty;
            }
            // sysinfo fallback
            let name = System::name().unwrap_or_else(|| "Linux".into());
            let ver = System::os_version().unwrap_or_default();
            format!("{} {}", name, ver).trim().to_string()
        }
        Platform::Windows => {
            let name = System::name().unwrap_or_else(|| "Windows".into());
            let ver = System::os_version().unwrap_or_default();
            format!("{} {}", name, ver).trim().to_string()
        }
        Platform::Unsupported => "Unknown OS".into(),
    }
}

// ─── Init system ─────────────────────────────────────────────────────

pub fn init_system(platform: &Platform) -> String {
    match platform {
        Platform::Android => "init.rc".into(),
        Platform::MacOS => "launchd".into(),
        Platform::Windows => "wininit".into(),
        Platform::Linux | Platform::FreeBSD => {
            // PID 1 comm
            if let Ok(comm) = fs::read_to_string("/proc/1/comm") {
                let name = comm.trim();
                if !name.is_empty() {
                    // Also give the user the more specific name when the
                    // binary is "init" but a well-known init lives on disk
                    if name == "init" {
                        if Path::new("/sbin/openrc").exists() {
                            return "openrc".into();
                        }
                        if Path::new("/sbin/dinit").exists() {
                            return "dinit".into();
                        }
                        if Path::new("/run/runit").exists() || Path::new("/etc/runit").exists() {
                            return "runit".into();
                        }
                    }
                    return name.to_string();
                }
            }
            if Path::new("/sbin/openrc").exists() {
                return "openrc".into();
            }
            if Path::new("/sbin/dinit").exists() {
                return "dinit".into();
            }
            "unknown".into()
        }
        Platform::Unsupported => "unknown".into(),
    }
}

// ─── Shell ───────────────────────────────────────────────────────────

pub fn shell() -> String {
    // $SHELL is the login shell
    if let Ok(sh) = env::var("SHELL")
        && let Some(name) = sh.rsplit('/').next()
        && !name.is_empty()
    {
        return name.to_string();
    }

    // Walk the process tree via sysinfo
    let sys = System::new_all();
    if let Ok(pid) = sysinfo::get_current_pid() {
        let mut cur = pid;
        for _ in 0..8 {
            if let Some(proc_) = sys.process(cur) {
                let name = proc_.name().to_string_lossy().to_string();
                if is_known_shell(&name) {
                    return name;
                }
                match proc_.parent() {
                    Some(p) => cur = p,
                    None => break,
                }
            } else {
                break;
            }
        }
    }

    "unknown".into()
}

fn is_known_shell(name: &str) -> bool {
    matches!(
        name,
        "bash"
            | "zsh"
            | "fish"
            | "sh"
            | "dash"
            | "ksh"
            | "mksh"
            | "tcsh"
            | "csh"
            | "elvish"
            | "nu"
            | "nushell"
            | "ion"
            | "xonsh"
            | "oil"
            | "osh"
            | "pwsh"
            | "powershell"
    )
}

// ─── Memory ──────────────────────────────────────────────────────────

pub fn memory(sys: &System) -> String {
    let total_mb = sys.total_memory() / 1_048_576;
    let used_mb = sys.used_memory() / 1_048_576;
    format!("{} / {} MB", used_mb, total_mb)
}

// ─── Uptime ──────────────────────────────────────────────────────────

pub fn uptime() -> String {
    // /proc/uptime is the most precise on Linux
    let secs = fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|c| c.split_whitespace().next()?.parse::<f64>().ok())
        .map(|f| f as u64)
        .unwrap_or_else(System::uptime);

    let d = secs / 86400;
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;

    let mut parts = Vec::with_capacity(3);
    if d > 0 {
        parts.push(format!("{} day{}", d, plural(d)));
    }
    if h > 0 {
        parts.push(format!("{} hour{}", h, plural(h)));
    }
    if m > 0 || parts.is_empty() {
        parts.push(format!("{} min{}", m, plural(m)));
    }
    parts.join(", ")
}

fn plural(n: u64) -> &'static str {
    if n == 1 { "" } else { "s" }
}

// ─── Storage ─────────────────────────────────────────────────────────

#[allow(clippy::useless_conversion)] // statvfs returns u32 on Android but u64 on other platforms
pub fn storage(platform: &Platform) -> String {
    let mount = match platform {
        Platform::Android => "/data",
        _ => "/",
    };

    // Primary: nix statvfs (POSIX, no shell-out)
    if let Ok(st) = statvfs(mount) {
        let bsize = u64::from(st.block_size());
        let total = u64::from(st.blocks()).saturating_mul(bsize);
        let avail = u64::from(st.blocks_available()).saturating_mul(bsize);
        let used = total.saturating_sub(avail);
        return format_bytes_pair(used, total);
    }

    // Fallback: sysinfo Disks
    let disks = Disks::new_with_refreshed_list();
    for d in disks.list() {
        if d.mount_point() == Path::new(mount) {
            let total = d.total_space();
            let avail = d.available_space();
            return format_bytes_pair(total - avail, total);
        }
    }

    "Unknown".into()
}

fn format_bytes_pair(used: u64, total: u64) -> String {
    const GIB: f64 = 1_073_741_824.0;
    format!("{:.1}G / {:.1}G", used as f64 / GIB, total as f64 / GIB)
}

// ─── DE / WM ────────────────────────────────────────────────────────

pub fn de_wm() -> String {
    // Environment variables (set by most session managers / compositors)
    for var in &[
        "XDG_CURRENT_DESKTOP",
        "DESKTOP_SESSION",
        "XDG_SESSION_DESKTOP",
    ] {
        if let Ok(val) = env::var(var) {
            let wm = if *var == "XDG_CURRENT_DESKTOP" {
                // May be "ubuntu:GNOME" — take part after last colon
                val.rsplit(':').next().unwrap_or(&val).to_string()
            } else {
                val
            };
            if !wm.is_empty() {
                return wm;
            }
        }
    }

    // Scan running processes for known window managers / compositors
    static KNOWN_WMS: &[&str] = &[
        "sway",
        "hyprland",
        "river",
        "niri",
        "wayfire",
        "kiwmi",
        "labwc",
        "i3",
        "bspwm",
        "dwm",
        "awesome",
        "openbox",
        "xmonad",
        "qtile",
        "herbstluftwm",
        "fvwm",
        "sowm",
        "catwm",
        "2bwm",
        "monsterwm",
        "tinywm",
        "kwin_wayland",
        "kwin_x11",
        "kwin",
        "mutter",
        "marco",
        "xfwm4",
        "compiz",
        "enlightenment",
        "budgie-wm",
        "muffin",
        "cinnamon",
        "gala",
        "picom",
        "icewm",
        "fluxbox",
        "jwm",
        "pekwm",
        "spectrwm",
        "stumpwm",
        "wmaker",
        "berry",
        "leftwm",
    ];

    let sys = System::new_all();
    for proc_ in sys.processes().values() {
        let name = proc_.name().to_string_lossy().to_lowercase();
        for wm in KNOWN_WMS {
            if name == *wm {
                return wm.to_string();
            }
        }
    }

    "unknown".into()
}

// ─── Packages ────────────────────────────────────────────────────────

pub fn package_info(platform: &Platform) -> String {
    packages::collect(platform)
}

// ─── Android props ───────────────────────────────────────────────────

pub fn android_phone() -> String {
    // Try using getprop command first (more reliable on Android)
    let brand = get_prop_cmd("ro.product.brand")
        .or_else(|| get_prop_cmd("ro.product.system.brand"))
        .or_else(|| get_prop_cmd("ro.product.vendor.brand"))
        .or_else(|| read_prop("/system/build.prop", "ro.product.brand"))
        .or_else(|| read_prop("/system/build.prop", "ro.product.system.brand"))
        .or_else(|| read_prop("/vendor/build.prop", "ro.product.brand"))
        .unwrap_or_else(|| "Unknown".into());

    // Try multiple property keys for model
    let model = get_prop_cmd("ro.product.model")
        .or_else(|| get_prop_cmd("ro.product.system.model"))
        .or_else(|| get_prop_cmd("ro.product.vendor.model"))
        .or_else(|| read_prop("/system/build.prop", "ro.product.model"))
        .or_else(|| read_prop("/system/build.prop", "ro.product.system.model"))
        .or_else(|| read_prop("/vendor/build.prop", "ro.product.model"))
        .unwrap_or_else(|| "Device".into());

    format!("{} {}", brand, model)
}

// Get Android property using getprop command
fn get_prop_cmd(key: &str) -> Option<String> {
    Command::new("getprop")
        .arg(key)
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

// ─── Helpers ─────────────────────────────────────────────────────────

fn read_prop(file: &str, key: &str) -> Option<String> {
    let content = fs::read_to_string(file).ok()?;
    let prefix = format!("{}=", key);
    for line in content.lines() {
        if let Some(val) = line.strip_prefix(&prefix) {
            let val = val.trim();
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

fn read_os_release_value(key: &str) -> Option<String> {
    let content = fs::read_to_string("/etc/os-release")
        .or_else(|_| fs::read_to_string("/usr/lib/os-release"))
        .ok()?;
    let prefix = format!("{}=", key);
    for line in content.lines() {
        if let Some(val) = line.strip_prefix(&prefix) {
            return Some(val.trim_matches('"').to_string());
        }
    }
    None
}
