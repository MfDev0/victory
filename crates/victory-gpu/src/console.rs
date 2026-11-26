//! Which console the title runs on. Titles pick settings (resolution,
//! memory budgets) by it, so the user chooses; Series X unless told.

use std::sync::atomic::{AtomicBool, Ordering};

const GIB: usize = 1 << 30;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Console {
    #[default]
    SeriesX,
    SeriesS,
}

impl Console {
    /// Title memory in bytes: (GPU-optimal, standard). Series X in the
    /// advanced configuration; Series S has one range, counted as standard
    /// (7,776 MiB, the published console-mode figure).
    // shortcut: later system updates gave Series S titles more (amount not
    // published here); the title sizes nothing from this figure
    // (it goes by the device type), so update it if a title does.
    pub const fn title_memory(self) -> (usize, usize) {
        match self {
            Console::SeriesX => (10 * GIB, 7 * GIB / 2),
            Console::SeriesS => (0, 7_776 << 20),
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Console::SeriesX => "Xbox Series X",
            Console::SeriesS => "Xbox Series S",
        }
    }
}

static SERIES_S: AtomicBool = AtomicBool::new(false);

/// Set before the title starts.
pub fn set(console: Console) {
    SERIES_S.store(console == Console::SeriesS, Ordering::Relaxed);
}

/// The dedicated memory of the card the host device is made on (the first
/// hardware adapter, as `D3D12CreateDevice` with no adapter picks it), in
/// MiB; `None` if DXGI will not say.
#[cfg(windows)]
pub fn card_memory_mb() -> Option<u64> {
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, IDXGIFactory1};
    // SAFETY: plain DXGI queries; nothing is created on the card.
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
        for i in 0..8 {
            let adapter = factory.EnumAdapters1(i).ok()?;
            let desc = adapter.GetDesc1().ok()?;
            if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 == 0 {
                return Some(desc.DedicatedVideoMemory as u64 >> 20);
            }
        }
    }
    None
}

#[cfg(not(windows))]
pub fn card_memory_mb() -> Option<u64> {
    None
}

/// The console to run as when Series X was asked for: Series S on a card
/// with 9 GiB of its own memory or less (6 and 8 GB cards), unless
/// `VICTORY_CONSOLE_AUTO=0`. Series X's render targets and memory took
/// 2.3 to 3 GB more of such a card than it has to give, and every texture
/// that did not fit then drew black (2026-11-25 and 11-26 reports: the
/// card at 5.8 GB of a 5.2 GB budget, 22,000 textures waiting). Series S
/// renders at a smaller size and the title asks for less memory. The
/// launcher's own switch to Series S never reached a running launcher that
/// had updated itself, so it is made here.
pub fn auto(asked: Console) -> (Console, Option<u64>) {
    if asked != Console::SeriesX || std::env::var_os("VICTORY_CONSOLE_AUTO").is_some_and(|v| v == "0") {
        return (asked, None);
    }
    match card_memory_mb() {
        Some(mb) if mb > 0 && mb <= 9 * 1024 => (Console::SeriesS, Some(mb)),
        _ => (asked, None),
    }
}

pub fn current() -> Console {
    if SERIES_S.load(Ordering::Relaxed) {
        Console::SeriesS
    } else {
        Console::SeriesX
    }
}
