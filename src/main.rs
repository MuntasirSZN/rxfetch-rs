mod display;
mod info;
mod packages;
mod platform;

use clap::Parser;
use platform::Platform;

/// A minimal system fetch tool written in Rust
#[derive(Parser, Debug)]
#[command(name = "rustfetch", version, about)]
struct Args {
    /// Disable color output
    #[arg(long, default_value_t = false)]
    no_color: bool,
}

fn main() {
    let args = Args::parse();
    let platform = Platform::detect();

    if !sysinfo::IS_SUPPORTED_SYSTEM {
        eprintln!(
            "Warning: sysinfo does not fully support this system ({:?}). \
             Memory, uptime, and process data may be unavailable.",
            platform
        );
    }

    let mut sys = sysinfo::System::new();
    sys.refresh_memory();

    display::setup_fonts(&platform);
    display::render(&args, &platform, &sys);
}
