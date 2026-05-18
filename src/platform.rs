#[cfg(target_os = "linux")]
use std::env;
#[cfg(target_os = "linux")]
use std::path::Path;

/// Every platform we know how to handle.
/// The `Unsupported` variant is the fallback for anything sysinfo
/// cannot instrument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Platform {
    Linux,
    Android,
    MacOS,
    FreeBSD,
    Windows,
    Unsupported,
}

impl Platform {
    /// Detect the current platform at runtime.
    ///
    /// Android is checked first because it also reports `target_os = "linux"`.
    /// Android detection is performed before checking `sysinfo::IS_SUPPORTED_SYSTEM`
    /// because sysinfo may not fully support Android but we can still detect it.
    pub fn detect() -> Self {
        // Android detection: target_os = "android" OR Termux prefix or /system/build.prop
        #[cfg(target_os = "android")]
        {
            Self::Android
        }

        #[cfg(not(target_os = "android"))]
        {
            #[cfg(target_os = "linux")]
            {
                let is_android = Path::new("/system/build.prop").exists()
                    || env::var("PREFIX")
                        .unwrap_or_default()
                        .contains("com.termux");
                if is_android {
                    return Self::Android;
                }
            }

            if !sysinfo::IS_SUPPORTED_SYSTEM {
                return Self::Unsupported;
            }

            if cfg!(target_os = "macos") {
                return Self::MacOS;
            }
            if cfg!(target_os = "freebsd") {
                return Self::FreeBSD;
            }
            if cfg!(target_os = "windows") {
                return Self::Windows;
            }
            if cfg!(target_os = "linux") {
                return Self::Linux;
            }

            Self::Unsupported
        }
    }

    /// `true` when the platform should show the colour palette strip.
    pub fn show_palette(self) -> bool {
        !matches!(self, Self::Android | Self::MacOS | Self::Unsupported)
    }

    /// `true` when a graphical DE/WM line should be shown.
    pub fn has_display() -> bool {
        std::env::var("DISPLAY")
            .or_else(|_| std::env::var("WAYLAND_DISPLAY"))
            .map(|v| !v.is_empty())
            .unwrap_or(false)
    }
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Linux => write!(f, "Linux"),
            Self::Android => write!(f, "Android"),
            Self::MacOS => write!(f, "macOS"),
            Self::FreeBSD => write!(f, "FreeBSD"),
            Self::Windows => write!(f, "Windows"),
            Self::Unsupported => write!(f, "Unsupported"),
        }
    }
}
