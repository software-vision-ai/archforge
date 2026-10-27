//! Build metadata: the commit and date come from the `ARCHFORGE_BUILD_SHA` and
//! `ARCHFORGE_BUILD_DATE` environment variables at compile time; without them the binary reports a
//! dev build.
//!
//! On Windows with the MSVC toolchain, it also writes the version and icon resource and
//! hands it to the linker. The icon is `dist/icons/archforge.ico`, rendered by `packaging/icons.sh`.
//! With `ARCHFORGE_REQUIRE_RESOURCES=1`, set for release builds, a missing icon fails the build.

#[path = "src/winres.rs"]
#[allow(dead_code)]
mod winres;

use std::path::Path;

fn main() {
    for key in ["ARCHFORGE_BUILD_SHA", "ARCHFORGE_BUILD_DATE"] {
        println!("cargo:rerun-if-env-changed={key}");
        if let Ok(value) = std::env::var(key) {
            println!("cargo:rustc-env={key}={value}");
        }
    }
    println!("cargo:rerun-if-env-changed=ARCHFORGE_REQUIRE_RESOURCES");
    let windows = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows");
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|env| env == "msvc");
    if !(windows && msvc) {
        return;
    }
    let required = std::env::var("ARCHFORGE_REQUIRE_RESOURCES").is_ok_and(|v| v == "1");
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let icon_path = Path::new(&manifest).join("../../dist/icons/archforge.ico");
    println!("cargo:rerun-if-changed={}", icon_path.display());
    let icon = std::fs::read(&icon_path).ok();
    if icon.is_none() && required {
        println!("cargo:warning={} is missing; run packaging/icons.sh first (the release requires the icon)", icon_path.display());
        std::process::exit(1);
    }
    let version_text = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let Some(version) = winres::parse_version(&version_text) else {
        println!("cargo:warning=the package version {version_text} is not x.y.z");
        std::process::exit(1);
    };
    let strings = [
        ("CompanyName", "The Archforge contributors"),
        ("FileDescription", "Archforge"),
        ("FileVersion", version_text.as_str()),
        ("InternalName", "archforge"),
        ("OriginalFilename", "archforge.exe"),
        ("ProductName", "Archforge"),
        ("ProductVersion", version_text.as_str()),
    ];
    let res = match winres::resource_file(version, &strings, icon.as_deref()) {
        Ok(r) => r,
        Err(e) => {
            println!("cargo:warning=the Windows resource could not be built: {e}");
            std::process::exit(1);
        }
    };
    let out = Path::new(&std::env::var("OUT_DIR").unwrap_or_default()).join("archforge.res");
    if let Err(e) = std::fs::write(&out, res) {
        println!("cargo:warning=could not write {}: {e}", out.display());
        std::process::exit(1);
    }
    println!("cargo:rustc-link-arg-bins={}", out.display());
}
