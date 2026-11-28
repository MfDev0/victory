//! The title's photos (photo mode's SAVE PHOTO): the title asks the console's capture service for
//! a screenshot (`XAppCaptureTakeScreenshot`, victory-hle's `display.rs`), which said "not
//! supported", and the title then said it could not save the photo. Now the next presented
//! frame, as the title drew it (before the F1 menu, the panel and the notices), is saved full size
//! as `GTAVI-<date>-<time>.png` in `%USERPROFILE%\Downloads\VIctory-Photos` (`VICTORY_PHOTOS=<folder>`
//! names another folder; `VICTORY_PHOTOS=0` turns it off and the title is told "not supported" again).
//! The picture is written on a thread of its own, so the present waits only for the copy.

use std::path::PathBuf;
use std::sync::Mutex;

use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

/// The name the next present's photo is saved as, while one is asked for.
static ASKED: Mutex<Option<PathBuf>> = Mutex::new(None);

/// The folder photos go to; `None` with `VICTORY_PHOTOS=0`.
pub fn folder() -> Option<PathBuf> {
    match std::env::var("VICTORY_PHOTOS") {
        Ok(v) if v.trim() == "0" => None,
        Ok(v) if !v.trim().is_empty() => Some(PathBuf::from(v.trim())),
        _ => {
            let home = std::env::var_os("USERPROFILE").map(PathBuf::from).or_else(|| {
                std::env::var_os("LOCALAPPDATA").map(|l| PathBuf::from(l).join("victory"))
            })?;
            Some(home.join("Downloads").join("VIctory-Photos"))
        }
    }
}

/// Asks for a photo of the next presented frame: the name of the file it is saved as (the
/// title's capture id), or `None` if photos are off or the folder cannot be made.
pub fn ask() -> Option<String> {
    let folder = folder()?;
    if let Err(e) = std::fs::create_dir_all(&folder) {
        tracing::warn!(error = %e, folder = %folder.display(), "photo: the folder could not be made");
        return None;
    }
    let base = format!("GTAVI-{}", stamp());
    let mut name = format!("{base}.png");
    let mut n = 2;
    while folder.join(&name).exists() {
        name = format!("{base}-{n}.png");
        n += 1;
    }
    *ASKED.lock().unwrap_or_else(|e| e.into_inner()) = Some(folder.join(&name));
    tracing::info!(file = %folder.join(&name).display(), "photo asked: saved at the next present");
    Some(name)
}

/// At a present: the file a photo of this frame is saved as, if one was asked for (taken).
pub(crate) fn take() -> Option<PathBuf> {
    ASKED.lock().unwrap_or_else(|e| e.into_inner()).take()
}

/// The local date and time as `YYYYMMDD-HHMMSS`.
fn stamp() -> String {
    // SAFETY: a plain structure of integers; zero is a valid value.
    let mut t: windows_sys::Win32::Foundation::SYSTEMTIME = unsafe { std::mem::zeroed() };
    // SAFETY: fills a plain structure of ours.
    unsafe { windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut t) };
    format!("{:04}{:02}{:02}-{:02}{:02}{:02}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond)
}

/// Records a copy of `texture` (in the copy-source state, one level, one slice) into a readback
/// buffer on `list`.
pub(crate) fn read_back(
    device: &ID3D12Device,
    list: &ID3D12GraphicsCommandList,
    texture: &ID3D12Resource,
) -> Option<(ID3D12Resource, D3D12_PLACED_SUBRESOURCE_FOOTPRINT)> {
    let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
    let mut total = 0;
    // SAFETY: a live texture; outputs for one subresource.
    unsafe {
        let desc = texture.GetDesc();
        device.GetCopyableFootprints(&desc, 0, 1, 0, Some(&mut footprint), None, None, Some(&mut total));
    }
    let heap = D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_READBACK, ..Default::default() };
    let desc = D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
        Width: total,
        Height: 1,
        DepthOrArraySize: 1,
        MipLevels: 1,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
        ..Default::default()
    };
    let mut buffer: Option<ID3D12Resource> = None;
    // SAFETY: plain creation; an open list; locations naming live resources.
    unsafe {
        if let Err(e) = device.CreateCommittedResource(&heap, D3D12_HEAP_FLAG_NONE, &desc, D3D12_RESOURCE_STATE_COPY_DEST, None, &mut buffer) {
            tracing::warn!(error = %e, "photo not taken: no readback buffer");
            return None;
        }
        let buffer = buffer?;
        let to = D3D12_TEXTURE_COPY_LOCATION {
            pResource: std::mem::transmute_copy(&buffer),
            Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
            Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { PlacedFootprint: footprint },
        };
        let from = D3D12_TEXTURE_COPY_LOCATION {
            pResource: std::mem::transmute_copy(texture),
            Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
            Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 0 },
        };
        list.CopyTextureRegion(&to, 0, 0, 0, &from, None);
        Some((buffer, footprint))
    }
}

/// After the copy recorded by [`read_back`] has run (the list was waited for): the pixels are
/// taken out of the buffer and written to `path` as a PNG on a thread of its own.
pub(crate) fn save(path: PathBuf, buffer: &ID3D12Resource, footprint: &D3D12_PLACED_SUBRESOURCE_FOOTPRINT) {
    let f = footprint.Footprint;
    let (width, height, pitch) = (f.Width, f.Height, f.RowPitch as usize);
    let Some(kind) = Kind::of(f.Format) else {
        tracing::warn!(format = f.Format.0, "photo not saved: a picture format not known here");
        return;
    };
    let mut data = std::ptr::null_mut();
    // SAFETY: the copy finished (the list was waited for); the mapping spans the footprint's rows.
    let rows = unsafe {
        if buffer.Map(0, None, Some(&mut data)).is_err() || data.is_null() {
            tracing::warn!("photo not saved: the readback buffer could not be mapped");
            return;
        }
        let rows = std::slice::from_raw_parts(data.cast::<u8>(), pitch * height as usize).to_vec();
        buffer.Unmap(0, None);
        rows
    };
    let spawned = std::thread::Builder::new().name("victory-photo".into()).spawn(move || {
        let rgb = to_rgb(kind, width, height, pitch, &rows);
        match std::fs::write(&path, png(width, height, &rgb)) {
            Ok(()) => tracing::info!(file = %path.display(), width, height, "photo saved"),
            Err(e) => tracing::warn!(error = %e, file = %path.display(), "photo not saved"),
        }
    });
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "photo not saved: no thread");
    }
}

/// The pixel layouts a presented frame comes in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Rgba8,
    Bgra8,
    Rgb10a2,
    Rgba16f,
}

impl Kind {
    fn of(format: DXGI_FORMAT) -> Option<Kind> {
        Some(match format {
            DXGI_FORMAT_R8G8B8A8_UNORM | DXGI_FORMAT_R8G8B8A8_UNORM_SRGB | DXGI_FORMAT_R8G8B8A8_TYPELESS => Kind::Rgba8,
            DXGI_FORMAT_B8G8R8A8_UNORM | DXGI_FORMAT_B8G8R8A8_UNORM_SRGB | DXGI_FORMAT_B8G8R8A8_TYPELESS
            | DXGI_FORMAT_B8G8R8X8_UNORM | DXGI_FORMAT_B8G8R8X8_UNORM_SRGB => Kind::Bgra8,
            DXGI_FORMAT_R10G10B10A2_UNORM | DXGI_FORMAT_R10G10B10A2_TYPELESS => Kind::Rgb10a2,
            DXGI_FORMAT_R16G16B16A16_FLOAT | DXGI_FORMAT_R16G16B16A16_TYPELESS => Kind::Rgba16f,
            _ => return None,
        })
    }

    fn bytes(self) -> usize {
        if self == Kind::Rgba16f { 8 } else { 4 }
    }
}

/// Rows of `kind` pixels (`pitch` bytes apart) as packed 8-bit RGB.
fn to_rgb(kind: Kind, width: u32, height: u32, pitch: usize, rows: &[u8]) -> Vec<u8> {
    let (w, h) = (width as usize, height as usize);
    let mut rgb = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        let row = &rows[y * pitch..][..w * kind.bytes()];
        for p in row.chunks_exact(kind.bytes()) {
            match kind {
                Kind::Rgba8 => rgb.extend_from_slice(&p[..3]),
                Kind::Bgra8 => rgb.extend([p[2], p[1], p[0]]),
                Kind::Rgb10a2 => {
                    let v = u32::from_le_bytes([p[0], p[1], p[2], p[3]]);
                    rgb.extend([v, v >> 10, v >> 20].map(|c| ((c & 0x3ff) >> 2) as u8));
                }
                Kind::Rgba16f => {
                    // Linear scene light, in SDR 0 to 1: the sRGB curve, clamped.
                    rgb.extend([0, 2, 4].map(|i| srgb(half(u16::from_le_bytes([p[i], p[i + 1]])))));
                }
            }
        }
    }
    rgb
}

/// An IEEE half as a float.
fn half(h: u16) -> f32 {
    let (sign, exp, frac) = ((h >> 15) as u32, ((h >> 10) & 0x1f) as u32, (h & 0x3ff) as u32);
    let mag = match exp {
        0 => frac as f32 / 1024.0 / 16384.0,
        31 => return if frac == 0 && sign == 0 { f32::MAX } else { 0.0 },
        _ => f32::from_bits(((exp + 112) << 23) | (frac << 13)),
    };
    if sign == 1 { -mag } else { mag }
}

/// Linear light 0 to 1 as an 8-bit sRGB value.
fn srgb(linear: f32) -> u8 {
    let c = if linear.is_nan() { 0.0 } else { linear.clamp(0.0, 1.0) };
    let s = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0 + 0.5) as u8
}

/// A PNG of `width * height` pixels of packed 8-bit RGB.
fn png(width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
    let row = width as usize * 3;
    let mut raw = Vec::with_capacity((row + 1) * height as usize);
    for line in rgb.chunks_exact(row.max(1)) {
        raw.push(0);
        raw.extend_from_slice(line);
    }
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::new();
    header.extend(width.to_be_bytes());
    header.extend(height.to_be_bytes());
    header.extend([8, 2, 0, 0, 0]); // 8-bit RGB
    chunk(&mut out, b"IHDR", &header);
    let mut compressed = vec![0u8; zlib_rs::compress_bound(raw.len())];
    let (idat, _) = zlib_rs::compress_slice(&mut compressed, &raw, zlib_rs::DeflateConfig::new(6));
    chunk(&mut out, b"IDAT", idat);
    chunk(&mut out, b"IEND", &[]);
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend((data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend(kind);
    out.extend(data);
    let crc = crc32(&out[start..]);
    out.extend(crc.to_be_bytes());
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & (crc & 1).wrapping_neg());
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixels_of_each_layout_come_out_as_rgb() {
        assert_eq!(to_rgb(Kind::Bgra8, 1, 1, 4, &[1, 2, 3, 255]), [3, 2, 1]);
        assert_eq!(to_rgb(Kind::Rgba8, 1, 1, 8, &[1, 2, 3, 255, 9, 9, 9, 9]), [1, 2, 3]);
        let v: u32 = 1023 | (512 << 10);
        assert_eq!(to_rgb(Kind::Rgb10a2, 1, 1, 4, &v.to_le_bytes()), [255, 128, 0]);
        // 1.0, 0.0, 2.0 as halves.
        let p = [0x3c00u16, 0, 0x4000, 0x3c00].iter().flat_map(|h| h.to_le_bytes()).collect::<Vec<_>>();
        assert_eq!(to_rgb(Kind::Rgba16f, 1, 1, 8, &p), [255, 0, 255]);
        assert_eq!(half(0x3800), 0.5);
        assert_eq!(Kind::of(DXGI_FORMAT_BC1_UNORM), None);
    }

    #[test]
    fn a_png_starts_and_ends_right() {
        let out = png(2, 1, &[255, 0, 0, 0, 255, 0]);
        assert!(out.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(out.ends_with(&[0xae, 0x42, 0x60, 0x82]));
        assert_eq!(crc32(b"IEND"), 0xae42_6082);
    }
}
