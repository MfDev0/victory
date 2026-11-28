//! The frame rate and the machine's load, drawn into the top left corner of
//! every frame shown.
//!
//! The window's caption has the frame rate, but a capture of the window's
//! picture (a stream) does not carry the caption. Here the numbers are part
//! of the picture: a small box of text copied onto the swap chain's back
//! buffer right after the title's frame, before it is presented. The
//! title's own frame is not touched, so saved frames and everything read
//! back from it are as before.
//!
//! The box has six lines, new once a second:
//!
//! ```text
//! 5.8 FPS  172 MS            the frames shown a second and a frame's time
//! CPU 31%  GAME 5.2 CORES    all processors, all programs; this process
//! TOP THREAD 97%             this process's busiest thread, of one processor
//! GPU 41%  GAME 38%          the card's busiest engine: all programs; this one
//! VRAM 9.0/10.0 GB           the card's memory this process uses / its budget
//! RAM 15.4 GB  FREE 3.1 GB   this process's committed memory; the PC's free
//! ```
//!
//! A thread near 100 % beside a card far from it is a frame held by that
//! thread; a card near 100 % holds it itself; neither busy is waiting (for
//! memory to cross between the two, mostly). The load comes from
//! `crate::stats`, the card's memory from the host.
//!
//! The text is drawn on the CPU with a font of its own (5 by 7 dots a
//! glyph) into one of four buffers the card copies from, taken in turn: a
//! buffer is written again only three texts (three seconds) after it was
//! last recorded, so a present still in the air reads what it was recorded
//! with. On by default; `VICTORY_FPS_COUNTER=fps` shows the frame rate
//! alone, `VICTORY_FPS_COUNTER=0` leaves the frame as the title drew it.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

/// The frame rate in tenths of a frame a second, as last measured (once a
/// second, by the presenter's meter); `u32::MAX` before the first.
static TENTHS: AtomicU32 = AtomicU32::new(u32::MAX);

/// What is drawn: nothing, the frame rate alone, or the whole box.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Off,
    Rate,
    All,
}

fn mode() -> Mode {
    static MODE: std::sync::OnceLock<Mode> = std::sync::OnceLock::new();
    *MODE.get_or_init(|| match std::env::var("VICTORY_FPS_COUNTER").as_deref() {
        Ok("0") => Mode::Off,
        Ok("fps") => Mode::Rate,
        _ => Mode::All,
    })
}

/// The frame rate just measured.
pub(crate) fn set(fps: f64) {
    if fps.is_finite() && fps >= 0.0 {
        TENTHS.store((fps * 10.0).round().min(99_999.0) as u32, Ordering::Relaxed);
    }
}

/// A glyph's seven rows, five dots each (bit 4 is the left one).
pub(crate) fn glyph(c: char) -> [u8; 7] {
    match c {
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        '.' => [0, 0, 0, 0, 0, 0b01100, 0b01100],
        '%' => [0b11001, 0b11010, 0b00010, 0b00100, 0b01000, 0b01011, 0b10011],
        '/' => [0b00001, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b10000],
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
        // For the settings menu (Try 6.24).
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        '-' => [0, 0, 0, 0b11111, 0, 0, 0],
        ':' => [0, 0b01100, 0b01100, 0, 0b01100, 0b01100, 0],
        '<' => [0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010],
        '>' => [0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000],
        '(' => [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010],
        ')' => [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000],
        ',' => [0, 0, 0, 0, 0b01100, 0b00100, 0b01000],
        _ => [0; 7],
    }
}

/// The text for a frame rate in tenths.
fn text(tenths: u32) -> String {
    format!("{}.{} FPS", tenths / 10, tenths % 10)
}

/// The glyphs a line of the whole box has room for.
const COLUMNS: usize = 26;

/// The glyphs of the frame rate alone ("9999.9 FPS").
const RATE_COLUMNS: usize = 10;

/// The six lines of the whole box (see the module's notes), each exactly
/// [`COLUMNS`] glyphs: for a frame rate in tenths, the machine's load if a
/// look was taken, and the card's memory in bytes (budget, used) if the
/// host answered. A line whose figures are not there yet is blank.
fn lines(tenths: u32, per: u32, stats: Option<crate::stats::Stats>, card: Option<(u64, u64)>) -> Vec<String> {
    let gb = |mb: f64| mb / 1024.0;
    let mut out = Vec::with_capacity(6);
    out.push(if tenths > 0 { format!("{}  {:.0} MS", text(tenths), 10_000.0 / f64::from(tenths)) } else { text(tenths) });
    if per > 1 && tenths > 0 {
        // DLSS Frame Generation shows `per` frames for each one rendered:
        // the rate shown beside the rate rendered (Try 6.24, as
        // trustno1-coder/VIctory-NVIDIA's "FPS: x FG: y").
        out[0].push_str(&format!("  FG {:.0}", f64::from(tenths) * f64::from(per) / 10.0));
    }
    match stats {
        Some(s) => {
            out.push(format!("CPU {:.0}%  GAME {:.1} CORES", s.cpu_all, s.cores));
            out.push(format!("TOP THREAD {:.0}%", s.top_thread));
            out.push(match (s.gpu_all, s.gpu_ours) {
                (Some(all), Some(ours)) => format!("GPU {all:.0}%  GAME {ours:.0}%"),
                _ => "GPU NOT KNOWN".to_owned(),
            });
        }
        None => out.extend([String::new(), String::new(), String::new()]),
    }
    out.push(match card {
        Some((budget, used)) => format!("VRAM {:.1}/{:.1} GB", gb((used >> 20) as f64), gb((budget >> 20) as f64)),
        None => String::new(),
    });
    out.push(match stats {
        Some(s) => format!("RAM {:.1} GB  FREE {:.1} GB", gb(f64::from(s.ram_ours_mb)), gb(f64::from(s.ram_free_mb))),
        None => String::new(),
    });
    out.into_iter().map(|line| fit(&line, COLUMNS)).collect()
}

/// `line` in exactly `columns` glyphs: cut, or filled with spaces.
pub(crate) fn fit(line: &str, columns: usize) -> String {
    let mut out: String = line.chars().take(columns).collect();
    out.extend(std::iter::repeat_n(' ', columns - out.chars().count()));
    out
}

/// Bytes a pixel of a swap chain format this layer presents in, or none.
fn pixel_bytes(format: DXGI_FORMAT) -> Option<usize> {
    match format {
        DXGI_FORMAT_R8G8B8A8_UNORM | DXGI_FORMAT_B8G8R8A8_UNORM | DXGI_FORMAT_R10G10B10A2_UNORM => Some(4),
        DXGI_FORMAT_R16G16B16A16_FLOAT => Some(8),
        _ => None,
    }
}

/// One pixel of `format`: of the text (`lit`) or of the box behind it.
fn pixel(format: DXGI_FORMAT, lit: bool, out: &mut [u8]) {
    // The text: a bright yellow. The box: nearly black.
    let (r, g, b): (f32, f32, f32) = if lit { (1.0, 0.92, 0.16) } else { (0.03, 0.03, 0.03) };
    let byte = |v: f32| (v * 255.0).round() as u8;
    match format {
        DXGI_FORMAT_R8G8B8A8_UNORM => out.copy_from_slice(&[byte(r), byte(g), byte(b), 255]),
        DXGI_FORMAT_B8G8R8A8_UNORM => out.copy_from_slice(&[byte(b), byte(g), byte(r), 255]),
        DXGI_FORMAT_R10G10B10A2_UNORM => {
            let ten = |v: f32| (v * 1023.0).round() as u32;
            out.copy_from_slice(&(ten(r) | ten(g) << 10 | ten(b) << 20 | 3 << 30).to_le_bytes());
        }
        _ => {
            for (i, v) in [r, g, b, 1.0].into_iter().enumerate() {
                out[i * 2..i * 2 + 2].copy_from_slice(&half(v).to_le_bytes());
            }
        }
    }
}

/// A value from 0 to 1 as a 16-bit float.
fn half(v: f32) -> u16 {
    let bits = v.clamp(0.0, 1.0).to_bits();
    if v <= 0.0 {
        return 0;
    }
    // Sign 0; the exponent moves from a bias of 127 to one of 15; the top
    // ten bits of the fraction stay. Values under 2^-14 (none of ours) are 0.
    let exponent = ((bits >> 23) & 0xFF) as i32 - 127 + 15;
    if exponent <= 0 {
        return 0;
    }
    ((exponent as u32) << 10 | (bits >> 13) & 0x3FF) as u16
}

/// The box for `text` (lines parted by `\n`) at `scale` dots a pixel: its
/// width and height in pixels and, row by row from the top, whether each
/// pixel is text. A glyph takes six dots across with its gap and a line
/// nine down with its gap; a pad of two dots lies all round.
fn bitmap(text: &str, scale: u32) -> (u32, u32, Vec<bool>) {
    let lines: Vec<Vec<char>> = text.split('\n').map(|l| l.chars().collect()).collect();
    let longest = lines.iter().map(Vec::len).max().unwrap_or(0).max(1) as u32;
    let pad = 2 * scale;
    let width = 2 * pad + longest * 6 * scale - scale;
    let height = 2 * pad + lines.len() as u32 * 9 * scale - 2 * scale;
    let mut lit = vec![false; (width * height) as usize];
    for (line, chars) in lines.iter().enumerate() {
        for (i, &c) in chars.iter().enumerate() {
            let rows = glyph(c);
            for (row, bits) in rows.into_iter().enumerate() {
                for column in 0..5u32 {
                    if bits >> (4 - column) & 1 == 0 {
                        continue;
                    }
                    let x0 = pad + (i as u32 * 6 + column) * scale;
                    let y0 = pad + (line as u32 * 9 + row as u32) * scale;
                    for y in y0..y0 + scale {
                        for x in x0..x0 + scale {
                            lit[(y * width + x) as usize] = true;
                        }
                    }
                }
            }
        }
    }
    (width, height, lit)
}

/// A buffer the card copies a box from, with its footprint and where its
/// bytes are mapped.
struct Made {
    buffer: ID3D12Resource,
    data: *mut u8,
    width: u32,
    height: u32,
    pitch: u32,
}

// SAFETY: a free-threaded D3D12 object, its mapping (written under
// `PANEL`'s lock only) and plain numbers.
unsafe impl Send for Made {}

/// The buffers a text is drawn into, in turn.
const RING: usize = 4;

/// Where on the picture a box goes.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Place {
    /// The top left corner, a margin in.
    Corner,
    /// The middle.
    Middle,
}

/// A box of text as last drawn: the box of numbers here, the settings
/// menu (`crate::menu`).
#[derive(Default)]
pub(crate) struct TextBox {
    /// What the buffers were made for: the format, the scale, the glyphs
    /// across and the lines.
    made_for: Option<(i32, u32, usize, usize)>,
    /// What is in `ring[at]`, as the caller keys it.
    shown: Option<u64>,
    ring: Vec<Made>,
    at: usize,
}

impl TextBox {
    /// Copies the box onto `back` (a copy's destination in `list`): the
    /// text `text` makes, drawn again only when `key` changes. Every text
    /// of one box is to have the same lines of `columns` glyphs (`fit`).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw(
        &mut self,
        host: &crate::host::Host,
        list: &ID3D12GraphicsCommandList,
        back: &ID3D12Resource,
        (width, height, format): (u32, u32, DXGI_FORMAT),
        (columns, rows): (usize, usize),
        key: u64,
        text: impl FnOnce() -> String,
        place: Place,
    ) {
        let Some(bytes) = pixel_bytes(format) else {
            return;
        };
        // One dot of the font is `scale` pixels: 2 at 720p, 3 at 1080p, 6 at 4K.
        let scale = (height / 360).clamp(2, 12);
        let made_for = Some((format.0, scale, columns, rows));
        if self.made_for != made_for {
            // (A buffer still read by a present in the air is not let go:
            // a change of size or format happens a few times a run at most.)
            std::mem::forget(std::mem::take(&mut self.ring));
            *self = TextBox { made_for, ..Default::default() };
        }
        if self.shown != Some(key) {
            let (w, h, lit) = bitmap(&text(), scale);
            let pitch = (w as usize * bytes).next_multiple_of(D3D12_TEXTURE_DATA_PITCH_ALIGNMENT as usize);
            let at = if self.ring.len() < RING {
                let Some((buffer, data)) = host.shared_buffer((pitch * h as usize) as u64) else {
                    return;
                };
                self.ring.push(Made { buffer, data, width: w, height: h, pitch: pitch as u32 });
                self.ring.len() - 1
            } else {
                (self.at + 1) % RING
            };
            let m = &self.ring[at];
            if (m.width, m.height, m.pitch) != (w, h, pitch as u32) {
                return;
            }
            // SAFETY: the buffer is mapped for the process's life and holds
            // `pitch * h` bytes; the card last read it RING texts ago.
            let out = unsafe { std::slice::from_raw_parts_mut(m.data, pitch * h as usize) };
            for y in 0..h as usize {
                for x in 0..w as usize {
                    let to = y * pitch + x * bytes;
                    pixel(format, lit[y * w as usize + x], &mut out[to..to + bytes]);
                }
            }
            self.at = at;
            self.shown = Some(key);
        }
        let Some(m) = self.ring.get(self.at) else {
            return;
        };
        let margin = 4 * scale;
        if m.width + 2 * margin > width || m.height + 2 * margin > height {
            return;
        }
        let (x, y) = match place {
            Place::Corner => (margin, margin),
            Place::Middle => ((width - m.width) / 2, (height - m.height) / 2),
        };
        // SAFETY: an open list; the back buffer is a copy's destination in it
        // and the box lies inside it; our buffer holds `pitch * height` bytes
        // and is kept for as long as the process runs.
        unsafe {
            let from = D3D12_TEXTURE_COPY_LOCATION {
                pResource: std::mem::transmute_copy(&m.buffer),
                Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    PlacedFootprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                        Offset: 0,
                        Footprint: D3D12_SUBRESOURCE_FOOTPRINT { Format: format, Width: m.width, Height: m.height, Depth: 1, RowPitch: m.pitch },
                    },
                },
            };
            let to = D3D12_TEXTURE_COPY_LOCATION {
                pResource: std::mem::transmute_copy(back),
                Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 0 },
            };
            list.CopyTextureRegion(&to, x, y, 0, &from, None);
        }
    }
}

static PANEL: Mutex<Option<TextBox>> = Mutex::new(None);

/// What F3 shows now (Try 6.24, the key from trustno1-coder/VIctory-NVIDIA's
/// stats panel): 0 the start's (`VICTORY_FPS_COUNTER`), then each press
/// goes on to the next of the whole box, the frame rate alone, nothing.
static PRESSED: AtomicU32 = AtomicU32::new(0);

/// F3 was pressed (`crate::menu`).
pub(crate) fn next_mode() {
    PRESSED.fetch_add(1, Ordering::Relaxed);
}

fn mode_now() -> Mode {
    let order = [Mode::All, Mode::Rate, Mode::Off];
    let start = order.iter().position(|&m| m == mode()).unwrap_or(0);
    order[(start + PRESSED.load(Ordering::Relaxed) as usize) % order.len()]
}

/// Copies the box onto the back buffer `back` (a copy's destination in
/// `list`), in the format and size it was made with.
pub(crate) fn draw(host: &crate::host::Host, list: &ID3D12GraphicsCommandList, back: &ID3D12Resource, dims: (u32, u32, DXGI_FORMAT)) {
    let mode = mode_now();
    if mode == Mode::Off {
        return;
    }
    let tenths = TENTHS.load(Ordering::Relaxed);
    if tenths == u32::MAX {
        return;
    }
    let stats = (mode == Mode::All).then(|| {
        crate::stats::start();
        crate::stats::now()
    });
    let look = stats.flatten();
    #[cfg(windows)]
    let per = crate::streamline::shown_per_frame();
    #[cfg(not(windows))]
    let per = 1;
    // The text changes with the rate, the look at the load and the frames
    // frame generation shows for each.
    let key = u64::from(tenths) | look.map_or(0, |(number, _)| number) << 20 | u64::from(per) << 60;
    let size = if mode == Mode::All { (COLUMNS, 6) } else { (RATE_COLUMNS, 1) };
    let make = || {
        if mode == Mode::All {
            let card = host.video_memory().map(|m| (m[0], m[1]));
            lines(tenths, per, look.map(|(_, s)| s), card).join("\n")
        } else {
            fit(&text(tenths), RATE_COLUMNS)
        }
    };
    let mut panel = PANEL.lock().unwrap_or_else(|e| e.into_inner());
    panel.get_or_insert_with(TextBox::default).draw(host, list, back, dims, size, key, make, Place::Corner);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_text_is_the_rate_to_a_tenth() {
        assert_eq!(text(103), "10.3 FPS");
        assert_eq!(text(7), "0.7 FPS");
        assert_eq!(text(300), "30.0 FPS");
    }

    #[test]
    fn the_box_holds_every_glyph_with_a_margin() {
        let (w, h, lit) = bitmap("10.3 FPS", 3);
        // Eight glyphs of five dots and seven gaps of one, a pad of two dots all round.
        assert_eq!((w, h), ((8 * 5 + 7 + 4) * 3, (7 + 4) * 3));
        assert_eq!(lit.len(), (w * h) as usize);
        // The pad is box, not text; the "1" has its stem lit.
        assert!(lit[..(w * 6) as usize].iter().all(|&l| !l), "the top pad");
        assert!((0..h).all(|y| !lit[(y * w) as usize] && !lit[(y * w + w - 1) as usize]), "the side pads");
        let at = |x: u32, y: u32| lit[(y * w + x) as usize];
        // Glyph 0 ("1"): dot column 2 of row 0 is lit, column 0 is not.
        assert!(at(6 + 2 * 3, 6) && !at(6, 6));
        // A space lights nothing: glyph 4.
        assert!((0..7 * 3).all(|y| (0..5 * 3).all(|x| !at(6 + 4 * 18 + x, 6 + y))));
        // Every glyph used has at least one dot, but the space.
        for c in "0123456789.FPS".chars() {
            assert!(glyph(c).iter().any(|&r| r != 0), "{c}");
        }
    }

    #[test]
    fn the_whole_box_has_six_lines_of_one_length_and_a_glyph_for_every_letter() {
        let stats = crate::stats::Stats {
            cpu_all: 100.0,
            cores: 28.0,
            top_thread: 100.0,
            gpu_all: Some(100.0),
            gpu_ours: Some(100.0),
            ram_ours_mb: 31 * 1024 + 512,
            ram_free_mb: 12 * 1024 + 100,
        };
        // The longest figures there can be, the usual ones, and none.
        let texts = [
            lines(99_999, 1, Some(stats), Some((12_282 << 20, 11_900 << 20))),
            lines(58, 1, Some(crate::stats::Stats { gpu_all: None, gpu_ours: None, ..stats }), Some((10_048 << 20, 9_039 << 20))),
            lines(0, 1, None, None),
        ];
        assert_eq!(texts[0][0].trim_end(), "9999.9 FPS  0 MS");
        assert_eq!(texts[0][1].trim_end(), "CPU 100%  GAME 28.0 CORES");
        assert_eq!(texts[0][5].trim_end(), "RAM 31.5 GB  FREE 12.1 GB");
        assert_eq!(texts[1][0].trim_end(), "5.8 FPS  172 MS");
        assert_eq!(texts[1][2].trim_end(), "TOP THREAD 100%");
        assert_eq!(texts[1][3].trim_end(), "GPU NOT KNOWN");
        assert_eq!(texts[1][4].trim_end(), "VRAM 8.8/9.8 GB");
        assert_eq!(texts[2][0].trim_end(), "0.0 FPS");
        // With frame generation at 2x: the rate shown beside it.
        assert_eq!(lines(280, 2, None, None)[0].trim_end(), "28.0 FPS  36 MS  FG 56");
        for lines in &texts {
            assert_eq!(lines.len(), 6);
            for line in lines {
                assert_eq!(line.chars().count(), COLUMNS, "{line:?}");
                for c in line.chars().filter(|c| *c != ' ') {
                    assert!(glyph(c).iter().any(|&r| r != 0), "no glyph for {c:?} in {line:?}");
                }
            }
        }
        // Six lines: nine dots a line, less the last gap, and the pad.
        let (w, h, lit) = bitmap(&texts[0].join("\n"), 3);
        assert_eq!((w, h), ((COLUMNS as u32 * 6 - 1 + 4) * 3, (6 * 9 - 2 + 4) * 3));
        assert_eq!(lit.len(), (w * h) as usize);
        // The frame rate alone keeps its length whatever the rate.
        assert_eq!(fit(&text(7), RATE_COLUMNS).len(), fit(&text(99_999), RATE_COLUMNS).len());
    }

    #[test]
    fn pixels_are_in_the_formats_the_layer_presents_in() {
        let mut four = [0u8; 4];
        pixel(DXGI_FORMAT_R8G8B8A8_UNORM, true, &mut four);
        assert_eq!(four, [255, 235, 41, 255]);
        pixel(DXGI_FORMAT_B8G8R8A8_UNORM, true, &mut four);
        assert_eq!(four, [41, 235, 255, 255]);
        pixel(DXGI_FORMAT_R10G10B10A2_UNORM, true, &mut four);
        let v = u32::from_le_bytes(four);
        assert_eq!((v & 1023, v >> 10 & 1023, v >> 20 & 1023, v >> 30), (1023, 941, 164, 3));
        let mut eight = [0u8; 8];
        pixel(DXGI_FORMAT_R16G16B16A16_FLOAT, true, &mut eight);
        assert_eq!(u16::from_le_bytes([eight[0], eight[1]]), 0x3C00, "1.0 as a half");
        assert_eq!(u16::from_le_bytes([eight[6], eight[7]]), 0x3C00);
        assert_eq!(half(0.5), 0x3800);
        assert_eq!(half(0.0), 0);
        assert_eq!(pixel_bytes(DXGI_FORMAT_R16G16B16A16_FLOAT), Some(8));
        assert_eq!(pixel_bytes(DXGI_FORMAT_D32_FLOAT), None);
    }
}
