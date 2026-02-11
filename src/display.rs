use crate::Args;
use crate::info;
use crate::platform::Platform;
use owo_colors::OwoColorize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use sysinfo::System;

// ─── Color abstraction ──────────────────────────────────────────────

#[derive(Clone, Copy)]
enum Color {
    Magenta,
    Green,
    Blue,
    Red,
    Yellow,
    Cyan,
}

fn paint_label(label: &str, color: Color, no_color: bool) -> String {
    if no_color {
        return label.to_string();
    }
    match color {
        Color::Magenta => format!("{}", label.magenta().bold()),
        Color::Green => format!("{}", label.green().bold()),
        Color::Blue => format!("{}", label.blue().bold()),
        Color::Red => format!("{}", label.red().bold()),
        Color::Yellow => format!("{}", label.yellow().bold()),
        Color::Cyan => format!("{}", label.cyan().bold()),
    }
}

fn paint_value(text: &str, no_color: bool) -> String {
    if no_color {
        text.to_string()
    } else {
        format!("{}", text.white().bold())
    }
}

// ─── ASCII cat ──────────────────────────────────────────────────────

fn ascii_line(row: usize, no_color: bool) -> String {
    if no_color {
        return match row {
            1 => "     •_•       ",
            2 => "     oo|       ",
            3 => r"    / '\       ",
            4 => r"   (\_;/)      ",
            _ => "               ",
        }
        .to_string();
    }

    match row {
        1 => format!(
            "     {}{}{}       ",
            "•".white().bold(),
            "_".on_black().black().bold(),
            "•".white().bold(),
        ),
        2 => format!(
            "     {}{}{}       ",
            "".on_black().bold(),
            "oo".on_yellow().yellow().bold(),
            "|".on_black().bold(),
        ),
        3 => format!(
            "    {}{}{}{}      ",
            "/".on_black().bold(),
            " ".on_white().bold(),
            "'".on_black().bold(),
            r"\".on_black().bold(),
        ),
        4 => format!(
            "   {}{}{}{}      ",
            "(\\".on_yellow().yellow().bold(),
            "_;/".on_black().bold(),
            ")".on_yellow().yellow().bold(),
            "".white(),
        ),
        _ => "               ".to_string(),
    }
}

// ─── Printing helpers ───────────────────────────────────────────────

fn info_line(label: &str, value: &str, no_color: bool, color: Color) {
    println!(
        "               {}  {}",
        paint_label(&format!("{:<5}", label), color, no_color),
        paint_value(value, no_color),
    );
}

fn art_line(row: usize, label: &str, value: &str, no_color: bool, color: Color) {
    println!(
        "{}{}  {}",
        ascii_line(row, no_color),
        paint_label(&format!("{:<5}", label), color, no_color),
        paint_value(value, no_color),
    );
}

// ─── Main render ────────────────────────────────────────────────────

pub fn render(args: &Args, platform: &Platform, sys: &System) {
    let nc = args.no_color;

    println!();

    if *platform == Platform::Android {
        info_line("phone", &info::android_phone(), nc, Color::Red);
    }

    let os_arch = if *platform == Platform::Android {
        info::linux_arch()
    } else {
        info::arch()
    };

    info_line(
        "os",
        &format!("{} {}", info::distro_name(platform), os_arch),
        nc,
        Color::Magenta,
    );
    info_line("ker", &info::kernel_release(), nc, Color::Green);

    art_line(1, "pkgs", &info::package_info(platform), nc, Color::Cyan);
    art_line(2, "sh", &info::shell(), nc, Color::Blue);
    art_line(3, " ram", &info::memory(sys), nc, Color::Yellow);
    art_line(4, "init", &info::init_system(platform), nc, Color::Magenta);

    if Platform::has_display() {
        info_line("de/wm", &info::de_wm(), nc, Color::Green);
    }

    info_line("up", &info::uptime(), nc, Color::Cyan);
    info_line("disk", &info::storage(platform), nc, Color::Yellow);

    println!();

    if platform.show_palette() {
        if nc {
            println!("        *  *  *  *  *");
        } else {
            println!(
                "        {}  {}  {}  {}  {}",
                "󰮯".yellow(),
                "󰊠".green(),
                "󰊠".blue(),
                "󰊠".red(),
                "󰊠".cyan(),
            );
        }
    }

    println!();
}

// ─── Font setup (pure file ops) ─────────────────────────────────────

pub fn setup_fonts(platform: &Platform) {
    if *platform == Platform::Android {
        return;
    }

    let home = match env::var("HOME") {
        Ok(h) => h,
        Err(_) => return,
    };

    let font_dir = PathBuf::from(&home).join(".local/share/fonts");

    // Already present → nothing to do
    if font_dir.join("Material.ttf").exists() {
        return;
    }

    let src = Path::new("ttf-material-design-icons");
    if !src.exists() {
        return;
    }

    let _ = fs::create_dir_all(&font_dir);
    if let Ok(entries) = fs::read_dir(src) {
        for entry in entries.flatten() {
            let dst = font_dir.join(entry.file_name());
            let _ = fs::copy(entry.path(), dst);
        }
    }
    // Fonts will be picked up on the next fontconfig cache rebuild.
}
