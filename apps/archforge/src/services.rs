//! The host services the shell may call: the installed fonts and the account's name.

use archforge_ui_egui::Services;

pub fn native() -> Services {
    Services {
        // The installed fonts, scanned and read on the shell's font worker.
        fonts: Some(std::sync::Arc::new(crate::fonts::SystemFonts::installed())),
        user_name: user_name(),
    }
}

/// The full name of the account the app runs under, for Home's avatar: on macOS what `id -F`
/// prints, on Linux the name field of the account's passwd entry, elsewhere the login name. None
/// when it cannot be read; the avatar is then hidden.
pub fn user_name() -> Option<String> {
    let name = full_name().or_else(|| std::env::var("USER").or_else(|_| std::env::var("USERNAME")).ok())?;
    let name = name.trim();
    // A name is a line of a few words; anything else is not shown.
    (!name.is_empty() && name.len() <= 256 && !name.contains('\n')).then(|| name.to_string())
}

#[cfg(target_os = "macos")]
fn full_name() -> Option<String> {
    let out = std::process::Command::new("/usr/bin/id").arg("-F").output().ok()?;
    if !out.status.success() || out.stdout.len() > 1024 {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

#[cfg(target_os = "linux")]
fn full_name() -> Option<String> {
    use std::io::Read as _;
    let user = std::env::var("USER").ok()?;
    let mut text = String::new();
    std::fs::File::open("/etc/passwd").ok()?.take(1 << 20).read_to_string(&mut text).ok()?;
    let gecos = text.lines().map(|l| l.split(':').collect::<Vec<_>>()).find(|f| f.first() == Some(&user.as_str()))?.get(4)?.to_string();
    gecos.split(',').next().map(str::to_string).filter(|n| !n.trim().is_empty())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn full_name() -> Option<String> {
    None
}
