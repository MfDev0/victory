//! Samplers as the host describes them, from the console's sampler
//! description (`CreateSamplerX`), which names the hardware's sampler
//! fields. (Immediate samplers in shader code come decoded with the
//! translation.)

use windows::Win32::Graphics::Direct3D12::*;

/// The fields both forms share.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Fields {
    /// Point 0, bilinear 1, anisotropic point 2, anisotropic bilinear 3.
    mag: u32,
    min: u32,
    /// None 0, point 1, linear 2.
    mip: u32,
    /// Plain 0, minimum 1, maximum 2.
    mode: u32,
    /// Host address modes.
    address: [u32; 3],
    anisotropy: u32,
    /// Host comparison function.
    compare: u32,
    min_lod: f32,
    max_lod: f32,
    bias: f32,
    border: [f32; 4],
}

impl Fields {
    fn host(&self) -> D3D12_SAMPLER_DESC {
        let linear = |f: u32| u32::from(f == 1 || f == 3);
        let aniso = self.mag >= 2 || self.min >= 2;
        // The console compares when the instruction says so, with the
        // sampler's function; the host needs a comparison sampler for that.
        // A sampler given a real test (less ... greater-or-equal) is one;
        // never, always and none are plain samplers.
        const LESS: u32 = 2;
        const GREATER_EQUAL: u32 = 7;
        let compares = (LESS..=GREATER_EQUAL).contains(&self.compare);
        let reduction = match self.mode {
            1 => 2,
            2 => 3,
            _ => u32::from(compares),
        };
        let filter = if aniso {
            0x55
        } else {
            linear(self.min) << 4 | linear(self.mag) << 2 | u32::from(self.mip == 2)
        };
        D3D12_SAMPLER_DESC {
            Filter: D3D12_FILTER((reduction << 7 | filter) as i32),
            AddressU: D3D12_TEXTURE_ADDRESS_MODE(self.address[0] as i32),
            AddressV: D3D12_TEXTURE_ADDRESS_MODE(self.address[1] as i32),
            AddressW: D3D12_TEXTURE_ADDRESS_MODE(self.address[2] as i32),
            MipLODBias: self.bias,
            MaxAnisotropy: if aniso {
                self.anisotropy.clamp(1, 16)
            } else {
                1
            },
            ComparisonFunc: D3D12_COMPARISON_FUNC(self.compare.max(1) as i32),
            BorderColor: self.border,
            MinLOD: self.min_lod,
            // Without a mip filter the console samples mip 0 only.
            MaxLOD: if self.mip == 0 {
                self.min_lod
            } else {
                self.max_lod
            },
        }
    }
}

/// Try 5.8, sharper surfaces (what PC drivers' and other emulators'
/// "anisotropic filtering override" and upscalers' LOD bias do):
/// - a sampler that filters linearly between levels (a textured surface,
///   not a data lookup: no point filter, no comparison, no minimum or
///   maximum) and has levels to filter between is made anisotropic, 16x
///   (`VICTORY_ANISO`, 1 to 16; 1: as the title asked). Roads, walls and
///   pavements seen at an angle are sampled from their sharp levels along
///   the line of sight instead of the blurred ones trilinear picks;
/// - the same samplers get a negative level bias where the title renders
///   smaller than the screen (`crate::dxgi::render_scale`): log2 of the
///   scale, -0.42 at 75 %, as DLSS, FSR and XeSS ask of the titles that use
///   them, so textures keep the sharpness of the screen's size
///   (`VICTORY_LOD_BIAS` sets the bias, in hundredths: -50 is -0.5).
pub(crate) fn enhance(desc: &mut D3D12_SAMPLER_DESC) {
    let (aniso, bias) = enhancement();
    let filter = desc.Filter.0 as u32;
    // Reduction (bits 7-8) plain; mip linear (bit 0) and min linear (bit
    // 4), or anisotropic already (bit 6).
    let plain = filter >> 7 == 0;
    let trilinear = filter & 0x11 == 0x11 || filter & 0x40 != 0;
    if !plain || !trilinear || desc.MaxLOD <= desc.MinLOD {
        return;
    }
    if aniso > 1 {
        desc.Filter = D3D12_FILTER_ANISOTROPIC;
        desc.MaxAnisotropy = desc.MaxAnisotropy.max(aniso);
    }
    if bias != 0.0 {
        desc.MipLODBias = (desc.MipLODBias + bias).clamp(-16.0, 15.99);
    }
}

/// The same for a static sampler of a root signature.
pub(crate) fn enhance_static(filter: &mut u32, max_anisotropy: &mut u32, bias: &mut f32, min_lod: f32, max_lod: f32) {
    let mut desc = D3D12_SAMPLER_DESC {
        Filter: D3D12_FILTER(*filter as i32),
        MaxAnisotropy: *max_anisotropy,
        MipLODBias: *bias,
        MinLOD: min_lod,
        MaxLOD: max_lod,
        ..Default::default()
    };
    enhance(&mut desc);
    *filter = desc.Filter.0 as u32;
    *max_anisotropy = desc.MaxAnisotropy;
    *bias = desc.MipLODBias;
}

/// The anisotropy and level bias of [`enhance`].
fn enhancement() -> (u32, f32) {
    static ONCE: std::sync::OnceLock<(u32, f32)> = std::sync::OnceLock::new();
    *ONCE.get_or_init(|| {
        let aniso = std::env::var("VICTORY_ANISO")
            .ok()
            .and_then(|v| v.trim().parse::<u32>().ok())
            .map_or(16, |n| n.clamp(1, 16));
        let bias = std::env::var("VICTORY_LOD_BIAS")
            .ok()
            .and_then(|v| v.trim().parse::<i32>().ok())
            .map_or_else(
                || (crate::dxgi::render_scale() as f32 / 100.0).log2(),
                |h| h.clamp(-300, 300) as f32 / 100.0,
            );
        tracing::info!(
            anisotropy = aniso,
            lod_bias = bias,
            "sharper surfaces: textured samplers made anisotropic, level bias for the render scale (VICTORY_ANISO, VICTORY_LOD_BIAS)"
        );
        (aniso, bias)
    })
}

/// The console's sampler description (27 words): flags, magnification,
/// minification, mip and depth filters, filter mode, three address modes,
/// minimum and maximum LOD, LOD bias, secondary bias, anisotropy, its bias
/// and threshold, comparison, border color, then tuning words.
pub(crate) fn console(words: &[u32; 27]) -> D3D12_SAMPLER_DESC {
    let f = |i: usize| f32::from_bits(words[i]);
    Fields {
        mag: words[1],
        min: words[2],
        mip: words[3],
        mode: words[5],
        address: [words[6], words[7], words[8]],
        min_lod: f(9),
        max_lod: f(10),
        bias: f(11),
        anisotropy: words[13],
        compare: words[16],
        border: [f(17), f(18), f(19), f(20)],
    }
    .host()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_descriptions_decode() {
        let mut w = [0u32; 27];
        w[1] = 1; // bilinear magnification
        w[2] = 0; // point minification
        w[3] = 2; // linear mips
        w[6..9].copy_from_slice(&[3, 3, 3]);
        w[10] = 1000.0f32.to_bits();
        let d = console(&w);
        assert_eq!(d.Filter.0, 0x5, "min point, mag and mip linear");
        assert_eq!(d.AddressW, D3D12_TEXTURE_ADDRESS_MODE_CLAMP);
        assert_eq!(d.MaxLOD, 1000.0);
        // A less-or-equal test makes a comparison sampler; never does not.
        w[16] = 4;
        assert_eq!(console(&w).Filter.0, 0x85);
        assert_eq!(console(&w).ComparisonFunc, D3D12_COMPARISON_FUNC_LESS_EQUAL);
        w[16] = 1;
        assert_eq!(console(&w).Filter.0, 0x5);
    }
}
