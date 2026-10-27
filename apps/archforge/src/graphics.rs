//! The graphics backend choice, the crash marker and the adapter choice.
//!
//! Each start runs a plan of choices. `Primary` is the platform's preferred backend: DX12 on
//! Windows (never OpenGL; Vulkan only when `ARCHFORGE_VULKAN=1`), Vulkan on Linux, Metal on macOS.
//! `Fallback` is the safe alternative: the software adapter on Windows, the GL backend on Linux, the
//! low-power adapter on macOS. A marker file names the choice being started and is removed once
//! frames have rendered; if it survives a launch, that choice crashed in the graphics stack, and
//! the next launch skips it. Within one run, a choice that fails to start is retried with the next.
//! On Linux the adapter driving a connected monitor is preferred, read from the kernel's connector
//! status.

use std::path::Path;
use std::sync::Arc;

use eframe::egui_wgpu::{WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew};
use eframe::wgpu;

/// Frames after which the start is considered healthy and the marker is removed.
pub const HEALTHY_AFTER_FRAMES: u64 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Primary,
    Fallback,
}

impl Choice {
    fn name(self) -> &'static str {
        match self {
            Choice::Primary => "primary",
            Choice::Fallback => "fallback",
        }
    }
}

/// The choices to try this launch, in order, given what the marker says about the last one.
pub fn plan(marker: &Path) -> Vec<Choice> {
    match std::fs::read_to_string(marker).ok().as_deref().map(str::trim) {
        Some("primary") => {
            log::warn!("the previous start crashed with the primary graphics backend; starting with the fallback");
            vec![Choice::Fallback]
        }
        Some(other) => {
            log::warn!("the previous start crashed with the {other} graphics backend; trying the primary again");
            vec![Choice::Primary, Choice::Fallback]
        }
        None => vec![Choice::Primary, Choice::Fallback],
    }
}

/// Records the choice being started.
pub fn begin(marker: &Path, choice: Choice) {
    if let Err(e) = craft_atomic::atomic_write(marker, choice.name().as_bytes()) {
        log::warn!("cannot write the graphics marker {}: {e}", marker.display());
    }
}

/// Removes the marker once the window has rendered a few frames.
pub fn healthy(marker: &Path) {
    if marker.exists()
        && let Err(e) = std::fs::remove_file(marker)
    {
        log::warn!("cannot remove the graphics marker {}: {e}", marker.display());
    }
}

fn backends(choice: Choice) -> wgpu::Backends {
    match (choice, cfg!(target_os = "windows"), cfg!(target_os = "linux")) {
        (_, true, _) => {
            let vulkan = std::env::var("ARCHFORGE_VULKAN").is_ok_and(|v| v == "1");
            if vulkan { wgpu::Backends::DX12 | wgpu::Backends::VULKAN } else { wgpu::Backends::DX12 }
        }
        (Choice::Primary, _, true) => wgpu::Backends::VULKAN,
        (Choice::Fallback, _, true) => wgpu::Backends::GL,
        _ => wgpu::Backends::PRIMARY,
    }
}

/// Configures the renderer for `choice`.
pub fn configure(options: &mut eframe::NativeOptions, choice: Choice) {
    let mut config = WgpuConfiguration::default();
    if let WgpuSetup::CreateNew(setup) = &mut config.wgpu_setup {
        configure_setup(setup, choice);
    }
    options.renderer = eframe::Renderer::Wgpu;
    options.wgpu_options = config;
}

fn configure_setup(setup: &mut WgpuSetupCreateNew, choice: Choice) {
    setup.instance_descriptor.backends = backends(choice);
    setup.power_preference = if choice == Choice::Fallback { wgpu::PowerPreference::LowPower } else { wgpu::PowerPreference::HighPerformance };
    if choice == Choice::Fallback && cfg!(target_os = "windows") {
        setup.native_adapter_selector = Some(Arc::new(|adapters: &[wgpu::Adapter], _surface: Option<&wgpu::Surface<'_>>| {
            adapters
                .iter()
                .find(|a| a.get_info().device_type == wgpu::DeviceType::Cpu)
                .or(adapters.first())
                .cloned()
                .ok_or_else(|| "no graphics adapter".to_string())
        }));
    } else if cfg!(target_os = "linux") {
        setup.native_adapter_selector = Some(Arc::new(|adapters: &[wgpu::Adapter], surface: Option<&wgpu::Surface<'_>>| {
            let infos: Vec<(u32, u32, bool)> = adapters
                .iter()
                .map(|a| {
                    let info = a.get_info();
                    (info.vendor, info.device, surface.is_none_or(|s| a.is_surface_supported(s)))
                })
                .collect();
            let index = choose_adapter(&infos, &monitor_driving_gpus()).ok_or_else(|| "no graphics adapter can present to the window".to_string())?;
            adapters.get(index).cloned().ok_or_else(|| "no graphics adapter".to_string())
        }));
    }
}

/// The index of the adapter to use: the first one that can present and drives a connected
/// monitor, else the first one that can present.
pub fn choose_adapter(adapters: &[(u32, u32, bool)], driving: &[(u32, u32)]) -> Option<usize> {
    let presentable = || adapters.iter().enumerate().filter(|(_, a)| a.2);
    presentable().find(|(_, a)| driving.contains(&(a.0, a.1))).or_else(|| presentable().next()).map(|(i, _)| i)
}

/// PCI vendor and device ids of the GPUs with a connected monitor, from the kernel's connector
/// status. Empty where the information does not exist.
fn monitor_driving_gpus() -> Vec<(u32, u32)> {
    let root = Path::new("/sys/class/drm");
    let Ok(entries) = std::fs::read_dir(root) else { return Vec::new() };
    let hex = |p: &Path| std::fs::read_to_string(p).ok().and_then(|s| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok());
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some((card, _connector)) = name.split_once('-') else { continue };
        let connected = std::fs::read_to_string(entry.path().join("status")).is_ok_and(|s| s.trim() == "connected");
        if connected
            && let (Some(vendor), Some(device)) = (hex(&root.join(card).join("device/vendor")), hex(&root.join(card).join("device/device")))
            && !out.contains(&(vendor, device))
        {
            out.push((vendor, device));
        }
    }
    out
}
