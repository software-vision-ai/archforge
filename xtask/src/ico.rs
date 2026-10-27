//! `cargo xtask ico <out.ico> <png>...`: packs rendered PNGs into a Windows icon file, PNG-encoded
//! entries, largest last (used by the Windows packaging script).

pub fn run(args: &[String]) -> Result<(), String> {
    let [out, pngs @ ..] = args else { return Err("usage: cargo xtask ico <out.ico> <png>...".into()) };
    if pngs.is_empty() {
        return Err("ico: give at least one PNG".into());
    }
    let mut images = Vec::new();
    for p in pngs {
        let bytes = std::fs::read(p).map_err(|e| format!("{p}: {e}"))?;
        if bytes.len() < 24 || bytes.get(1..4) != Some(b"PNG".as_slice()) {
            return Err(format!("{p}: not a PNG file"));
        }
        let dim = |at: usize| bytes.get(at..at + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]])).unwrap_or(0);
        let (w, h) = (dim(16), dim(20));
        if w == 0 || h == 0 || w > 256 || h > 256 {
            return Err(format!("{p}: {w}x{h}; icon images are 1 to 256 pixels square"));
        }
        images.push((w, h, bytes));
    }
    images.sort_by_key(|(w, _, _)| *w);
    let count = u16::try_from(images.len()).map_err(|_| "ico: too many images")?;
    let mut ico = Vec::new();
    ico.extend_from_slice(&[0, 0, 1, 0]);
    ico.extend_from_slice(&count.to_le_bytes());
    let mut offset = 6 + 16 * u32::from(count);
    for (w, h, bytes) in &images {
        let size = u32::try_from(bytes.len()).map_err(|_| "ico: image too large")?;
        ico.push(if *w == 256 { 0 } else { *w as u8 });
        ico.push(if *h == 256 { 0 } else { *h as u8 });
        ico.extend_from_slice(&[0, 0, 1, 0, 32, 0]);
        ico.extend_from_slice(&size.to_le_bytes());
        ico.extend_from_slice(&offset.to_le_bytes());
        offset += size;
    }
    for (_, _, bytes) in &images {
        ico.extend_from_slice(bytes);
    }
    std::fs::write(out, ico).map_err(|e| format!("{out}: {e}"))
}
