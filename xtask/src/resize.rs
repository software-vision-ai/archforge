//! `cargo xtask resize <in.png> <out.png> <size> [--grid]`: scales a square PNG down to `size`
//! pixels a side for the packaging scripts' icon sizes. Each pixel is the average of the source
//! pixels it covers, alpha premultiplied so a transparent edge stays clean. With `--grid` the
//! picture is set on the platform icon grid: its tile is 824/1024 of the side, centred, with a
//! clear margin round it.

use std::io::Cursor;

/// The largest file and side read, and the largest side written.
const MAX_FILE: usize = 64 * 1024 * 1024;
const MAX_SIDE: u32 = 8192;

pub fn run(args: &[String]) -> Result<(), String> {
    const USAGE: &str = "usage: cargo xtask resize <in.png> <out.png> <size> [--grid]";
    let grid = args.iter().any(|a| a == "--grid");
    let rest: Vec<&String> = args.iter().filter(|a| *a != "--grid").collect();
    let [input, output, size] = rest.as_slice() else { return Err(USAGE.into()) };
    let size: usize = size.parse().ok().filter(|s| (1..=MAX_SIDE as usize).contains(s)).ok_or_else(|| format!("{size}: not a size; {USAGE}"))?;
    let (side, rgba) = read(input)?;
    if size > side {
        return Err(format!("{input}: {side} pixels a side, smaller than {size}"));
    }
    let image = if grid {
        let tile = (size * 824).div_ceil(1024).min(size);
        let small = scale(&rgba, side, tile);
        let mut out = vec![0u8; size * size * 4];
        let at = (size - tile) / 2;
        for y in 0..tile {
            let from = y * tile * 4;
            let to = ((y + at) * size + at) * 4;
            out[to..to + tile * 4].copy_from_slice(&small[from..from + tile * 4]);
        }
        out
    } else {
        scale(&rgba, side, size)
    };
    write(output, size, &image)
}

/// A square PNG's side and its pixels as 8-bit RGBA.
fn read(path: &str) -> Result<(usize, Vec<u8>), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    if bytes.len() > MAX_FILE {
        return Err(format!("{path}: larger than {MAX_FILE} bytes"));
    }
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| format!("{path}: {e}"))?;
    let (w, h) = (reader.info().width, reader.info().height);
    if w != h || w == 0 || w > MAX_SIDE {
        return Err(format!("{path}: {w}x{h}; a square of 1 to {MAX_SIDE} pixels is needed"));
    }
    let mut buf = vec![0u8; reader.output_buffer_size().ok_or_else(|| format!("{path}: too large to read"))?];
    let frame = reader.next_frame(&mut buf).map_err(|e| format!("{path}: {e}"))?;
    let side = w as usize;
    let pixels = side * side;
    let rgba = match frame.color_type {
        png::ColorType::Rgba => buf[..pixels * 4].to_vec(),
        png::ColorType::Rgb => buf[..pixels * 3].chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => buf[..pixels * 2].chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        png::ColorType::Grayscale => buf[..pixels].iter().flat_map(|g| [*g, *g, *g, 255]).collect(),
        png::ColorType::Indexed => return Err(format!("{path}: the palette was not expanded")),
    };
    Ok((side, rgba))
}

/// The source pixels each of `to` pixels covers along `from`, with the share each gives.
fn weights(from: usize, to: usize) -> Vec<Vec<(usize, f32)>> {
    let step = from as f32 / to as f32;
    (0..to)
        .map(|i| {
            let (lo, hi) = (i as f32 * step, (i + 1) as f32 * step);
            let mut w = Vec::new();
            let mut j = lo.floor() as usize;
            while (j as f32) < hi && j < from {
                let cover = (hi.min(j as f32 + 1.0) - lo.max(j as f32)) / step;
                if cover > 0.0 {
                    w.push((j, cover));
                }
                j += 1;
            }
            w
        })
        .collect()
}

/// `rgba`, `from` pixels a side, averaged down to `to` pixels a side.
fn scale(rgba: &[u8], from: usize, to: usize) -> Vec<u8> {
    let w = weights(from, to);
    // Premultiplied, across first, then down.
    let mut across = vec![0f32; from * to * 4];
    for y in 0..from {
        for (x, ws) in w.iter().enumerate() {
            let mut acc = [0f32; 4];
            for &(j, k) in ws {
                let p = &rgba[(y * from + j) * 4..(y * from + j) * 4 + 4];
                let a = p[3] as f32 / 255.0;
                for c in 0..3 {
                    acc[c] += p[c] as f32 * a * k;
                }
                acc[3] += a * k;
            }
            across[(y * to + x) * 4..(y * to + x) * 4 + 4].copy_from_slice(&acc);
        }
    }
    let mut out = vec![0u8; to * to * 4];
    for (y, ws) in w.iter().enumerate() {
        for x in 0..to {
            let mut acc = [0f32; 4];
            for &(j, k) in ws {
                for c in 0..4 {
                    acc[c] += across[(j * to + x) * 4 + c] * k;
                }
            }
            let a = acc[3];
            let px = &mut out[(y * to + x) * 4..(y * to + x) * 4 + 4];
            for c in 0..3 {
                px[c] = if a > 0.0 { (acc[c] / a).round().clamp(0.0, 255.0) as u8 } else { 0 };
            }
            px[3] = (a * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

fn write(path: &str, side: usize, rgba: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{path}: {e}"))?;
    let side = u32::try_from(side).map_err(|_| format!("{path}: too large"))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), side, side);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| format!("{path}: {e}"))?;
    writer.write_image_data(rgba).map_err(|e| format!("{path}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_average_keeps_colour_and_coverage_and_a_clear_edge_stays_clean() {
        // Two by two: opaque red, opaque blue, and two clear pixels whose colour must not leak.
        let src = [255, 0, 0, 255, 0, 0, 255, 255, 0, 255, 0, 0, 0, 255, 0, 0];
        let one = scale(&src, 2, 1);
        assert_eq!(one, [128, 0, 128, 128]);
        assert_eq!(scale(&src, 2, 2), [255, 0, 0, 255, 0, 0, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0], "a clear pixel keeps no colour");
        let w = weights(5, 2);
        let total: f32 = w[0].iter().map(|(_, k)| k).sum();
        assert!((total - 1.0).abs() < 1e-5, "each pixel's shares make one: {w:?}");
    }
}
