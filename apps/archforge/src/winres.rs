//! A compiled Windows resource file (`.res`) holding the version information and, when there is
//! one, the application icon. The MSVC linker takes the file as an input directly, so no
//! resource compiler is needed. `build.rs` includes this module; the tests run on every platform.
//!
//! Layout, from the published resource-file format: an empty header, then one header and its
//! data per resource, each padded to four bytes. The version resource is a tree of
//! `{ length, value length, type, key, value, children }` nodes rooted at `VS_VERSION_INFO`.

const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;
const RT_VERSION: u16 = 16;
const LANG_EN_US: u16 = 0x0409;
const CODEPAGE_UNICODE: u16 = 1200;
const MOVEABLE_PURE: u16 = 0x0030;
const VS_FFI_SIGNATURE: u32 = 0xFEEF_04BD;

fn pad4(out: &mut Vec<u8>) {
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

fn utf16z(s: &str) -> Vec<u8> {
    s.encode_utf16().chain(std::iter::once(0)).flat_map(u16::to_le_bytes).collect()
}

fn resource(out: &mut Vec<u8>, kind: u16, name: u16, data: &[u8]) -> Result<(), String> {
    let size = u32::try_from(data.len()).map_err(|_| "a resource over 4 GiB".to_string())?;
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&32u32.to_le_bytes());
    out.extend_from_slice(&[0xFF, 0xFF]);
    out.extend_from_slice(&kind.to_le_bytes());
    out.extend_from_slice(&[0xFF, 0xFF]);
    out.extend_from_slice(&name.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&MOVEABLE_PURE.to_le_bytes());
    out.extend_from_slice(&(if kind == 0 { 0 } else { LANG_EN_US }).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(data);
    pad4(out);
    Ok(())
}

/// One node of the version tree. `value_len` is in bytes for binary values and in UTF-16 units
/// for text, as the format requires.
fn node(key: &str, value: &[u8], value_len: u16, text: bool, children: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    let mut out = vec![0, 0];
    out.extend_from_slice(&value_len.to_le_bytes());
    out.extend_from_slice(&u16::from(text).to_le_bytes());
    out.extend_from_slice(&utf16z(key));
    pad4(&mut out);
    out.extend_from_slice(value);
    for child in children {
        pad4(&mut out);
        out.extend_from_slice(child);
    }
    let len = u16::try_from(out.len()).map_err(|_| format!("the version node {key} is over 64 KiB"))?;
    out[..2].copy_from_slice(&len.to_le_bytes());
    Ok(out)
}

fn version_info(version: [u16; 4], strings: &[(&str, &str)]) -> Result<Vec<u8>, String> {
    let ms = (u32::from(version[0]) << 16) | u32::from(version[1]);
    let ls = (u32::from(version[2]) << 16) | u32::from(version[3]);
    let mut fixed = Vec::with_capacity(52);
    for word in [VS_FFI_SIGNATURE, 0x0001_0000, ms, ls, ms, ls, 0x3F, 0, 0x0004_0004, 1, 0, 0, 0] {
        fixed.extend_from_slice(&word.to_le_bytes());
    }
    let mut entries = Vec::new();
    for (key, value) in strings {
        let units = u16::try_from(value.encode_utf16().count() + 1).map_err(|_| format!("the version string {key} is too long"))?;
        entries.push(node(key, &utf16z(value), units, true, &[])?);
    }
    let table = node("040904B0", &[], 0, true, &entries)?;
    let string_info = node("StringFileInfo", &[], 0, true, &[table])?;
    let mut translation = Vec::new();
    translation.extend_from_slice(&LANG_EN_US.to_le_bytes());
    translation.extend_from_slice(&CODEPAGE_UNICODE.to_le_bytes());
    let var = node("Translation", &translation, 4, false, &[])?;
    let var_info = node("VarFileInfo", &[], 0, true, &[var])?;
    node("VS_VERSION_INFO", &fixed, 52, false, &[string_info, var_info])
}

/// The images of an `.ico` file: (width, height, colours, planes, bits, data).
fn icon_images(ico: &[u8]) -> Result<Vec<(u8, u8, u8, u16, u16, &[u8])>, String> {
    let word = |at: usize| ico.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let dword = |at: usize| ico.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    if word(0) != Some(0) || word(2) != Some(1) {
        return Err("not an icon file".into());
    }
    let count = usize::from(word(4).ok_or("a truncated icon file")?);
    let mut images = Vec::with_capacity(count.min(64));
    for i in 0..count {
        let at = 6 + 16 * i;
        let entry = ico.get(at..at + 16).ok_or("a truncated icon directory")?;
        let size = usize::try_from(dword(at + 8).ok_or("a truncated icon directory")?).map_err(|_| "an icon image too large")?;
        let offset = usize::try_from(dword(at + 12).ok_or("a truncated icon directory")?).map_err(|_| "an icon offset too large")?;
        let data = ico.get(offset..offset.checked_add(size).ok_or("an icon image past the end")?).ok_or("an icon image past the end")?;
        images.push((entry[0], entry[1], entry[2], word(at + 4).unwrap_or(1), word(at + 6).unwrap_or(32), data));
    }
    Ok(images)
}

/// The whole resource file.
pub fn resource_file(version: [u16; 4], strings: &[(&str, &str)], ico: Option<&[u8]>) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    resource(&mut out, 0, 0, &[])?;
    resource(&mut out, RT_VERSION, 1, &version_info(version, strings)?)?;
    if let Some(ico) = ico {
        let images = icon_images(ico)?;
        let mut group = Vec::new();
        group.extend_from_slice(&0u16.to_le_bytes());
        group.extend_from_slice(&1u16.to_le_bytes());
        group.extend_from_slice(&u16::try_from(images.len()).map_err(|_| "too many icon images")?.to_le_bytes());
        for (i, (w, h, colours, planes, bits, data)) in images.iter().enumerate() {
            let id = u16::try_from(i + 1).map_err(|_| "too many icon images")?;
            resource(&mut out, RT_ICON, id, data)?;
            group.extend_from_slice(&[*w, *h, *colours, 0]);
            group.extend_from_slice(&planes.to_le_bytes());
            group.extend_from_slice(&bits.to_le_bytes());
            group.extend_from_slice(&u32::try_from(data.len()).map_err(|_| "an icon image too large")?.to_le_bytes());
            group.extend_from_slice(&id.to_le_bytes());
        }
        resource(&mut out, RT_GROUP_ICON, 1, &group)?;
    }
    Ok(out)
}

/// `x.y.z` as the four numbers of a Windows version.
pub fn parse_version(v: &str) -> Option<[u16; 4]> {
    // A pre-release (0.2.0-rc.1) keeps its numbers; Windows' version resource holds no suffix.
    let numbers = v.split('-').next()?;
    let mut parts = numbers.split('.').map(|p| p.parse::<u16>().ok());
    Some([parts.next()??, parts.next()??, parts.next()??, 0])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(b: &[u8], at: usize) -> u16 {
        u16::from_le_bytes([b[at], b[at + 1]])
    }

    #[test]
    fn the_file_starts_with_the_empty_header_and_holds_the_version_tree() {
        let res = resource_file([1, 2, 3, 0], &[("ProductName", "Archforge"), ("FileVersion", "1.2.3")], None).unwrap();
        assert_eq!(res.len() % 4, 0);
        assert_eq!(&res[..8], &[0, 0, 0, 0, 32, 0, 0, 0]);
        let version = &res[32..];
        assert_eq!(word(version, 10), RT_VERSION);
        let data = &version[32..];
        let key = utf16z("VS_VERSION_INFO");
        assert_eq!(&data[6..6 + key.len()], &key[..]);
        let signature = VS_FFI_SIGNATURE.to_le_bytes();
        assert!(data.windows(4).any(|w| w == signature));
        assert_eq!(usize::from(word(data, 0)), u32::from_le_bytes([version[0], version[1], version[2], version[3]]) as usize);
        let text = utf16z("Archforge");
        assert!(data.windows(text.len()).any(|w| w == &text[..]));
    }

    #[test]
    fn an_icon_becomes_one_image_resource_per_size_and_a_group() {
        let png = [0x89u8, b'P', b'N', b'G', 1, 2, 3, 4];
        let mut ico = vec![0, 0, 1, 0, 1, 0, 16, 16, 0, 0, 1, 0, 32, 0];
        ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
        ico.extend_from_slice(&22u32.to_le_bytes());
        ico.extend_from_slice(&png);
        let res = resource_file([1, 0, 0, 0], &[], Some(&ico)).unwrap();
        assert!(res.windows(png.len()).any(|w| w == png));
        assert!(resource_file([1, 0, 0, 0], &[], Some(b"not an icon")).is_err());
        let mut truncated = ico.clone();
        truncated.truncate(24);
        assert!(resource_file([1, 0, 0, 0], &[], Some(&truncated)).is_err());
    }

    #[test]
    fn versions_parse_from_the_workspace_form() {
        assert_eq!(parse_version("1.2.3"), Some([1, 2, 3, 0]));
        assert_eq!(parse_version("1.x.3"), None);
        assert_eq!(parse_version("0.2.0-rc.1"), Some([0, 2, 0, 0]), "a pre-release keeps its numbers");
    }
}
