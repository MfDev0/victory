//! A least amount of indirect light.
//!
//! Grand Theft Auto VI's indirect diffuse light comes from probes it fills
//! by tracing rays, which do not run here yet: its program that resolves
//! them (`CS_ScreenProbes_TemporalResolve_*`) leaves next to nothing, and
//! whatever the sun does not reach directly is black. Right after that
//! program, its output is replaced by a constant (it used to be a floor
//! under what the program wrote, but that is garbage: see the shader). The
//! constant is the same everywhere: the light in these images does not go with the title's exposure
//! (well-lit surfaces hold about the same in a shot at -5.5 EV and in one at
//! -9.75). The title's own lighting then applies it with the surfaces'
//! colors. Raised, not added: the result is the next frame's history.
//!
//! `VICTORY_AMBIENT=<level>` sets it (0.45 by default, 0 for none); `off`
//! keeps the title's own output.
//! shortcut: the same light everywhere, from everywhere; gone once rays run.
//!
//! Try 6.25: during play the world gets trustno1-coder/VIctory-NVIDIA's
//! estimate from the frame instead, with the air and reflections
//! ([`estimate`]); cutscenes keep this constant (`crate::cutscene`).

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};
use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D::ID3DBlob;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::s;

use crate::host::{HostList, host};
use crate::object::This;

pub(crate) mod estimate;

/// The programs it follows, by the start of their name.
pub(crate) const PROGRAM: &[u8] = b"CS_ScreenProbes_TemporalResolve";

/// The title's programs the stand-ins follow (Try 6.25, from
/// trustno1-coder/VIctory-NVIDIA).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Role {
    /// Resolves the diffuse light ([`PROGRAM`]): raised to the constant or
    /// the estimate.
    Resolve,
    /// Keeps the light visible surfaces send out: the estimate's source.
    Radiance,
    /// The last program that writes what the air scatters.
    Air,
    /// Gives the reflection rays that found nothing the sky's light
    /// (`CS_RayMiss`, not `CS_RayMiss_Volumetrics` nor the screen probes').
    Miss,
}

/// The program's part, by its name in the container.
pub(crate) fn role(container: &[u8]) -> Option<Role> {
    let has = |name: &[u8]| container.windows(name.len()).any(|w| w == name);
    if has(PROGRAM) {
        Some(Role::Resolve)
    } else if has(b"CS_CacheScreenSpaceRadiance") {
        Some(Role::Radiance)
    } else if has(b"CS_AccumulateLocalLightInscatteringVolume") {
        Some(Role::Air)
    } else if is_miss(container) {
        Some(Role::Miss)
    } else {
        None
    }
}

/// `CS_RayMiss_` and the eight hex digits of a variant's hash.
fn is_miss(container: &[u8]) -> bool {
    const NAME: &[u8] = b"CS_RayMiss_";
    container.windows(NAME.len() + 9).any(|w| {
        w.starts_with(NAME) && w[NAME.len()..NAME.len() + 8].iter().all(u8::is_ascii_hexdigit) && w[NAME.len() + 8] == b'_'
    })
}

/// Where the other programs write what the estimate reads or changes: the
/// light of visible surfaces, what the air scatters, the reflected light
/// (R11G11B10, half the picture's size) and the depth the miss program reads
/// (32-bit, the picture's size): root parameter and place in its table.
pub(crate) const RADIANCE: (u8, u32) = (1, 16);
pub(crate) const AIR: (u8, u32) = (1, 32);
pub(crate) const MISS_LIGHT: (u8, u32) = (1, 104);
pub(crate) const MISS_DEPTH: (u8, u32) = (0, 1000);
/// The miss program's first root constant: which surfaces its rays are of.
pub(crate) const MISS_OPAQUE: u32 = 0;
pub(crate) const MISS_ALPHA: u32 = 1;
/// Where they write the indirect diffuse light (R9G9B9E5 as 32-bit
/// integers): root parameter and place in its table.
pub(crate) const LIGHT: (u8, u32) = (1, 176);

/// 0.6 looked balanced in run a2; Try 6.4: 0.45, as people came out too
/// bright with 0.6 (the level is the same in every shot, and the title
/// raises its exposure in dark ones, at night and indoors, which made skin
/// and clothes glow). `VICTORY_AMBIENT=off` leaves what the title's resolve
/// wrote; 0 is no indirect diffuse light at all.
const DEFAULT: f32 = 0.45;

/// The floor; `None` if off.
pub(crate) fn level() -> Option<f32> {
    static LEVEL: OnceLock<f32> = OnceLock::new();
    let level = *LEVEL.get_or_init(|| {
        let set = std::env::var("VICTORY_AMBIENT").unwrap_or_default();
        if set.trim() == "off" {
            return -1.0;
        }
        set.trim().parse().ok().filter(|l: &f32| l.is_finite() && *l >= 0.0).unwrap_or(DEFAULT)
    });
    (level >= 0.0).then_some(level)
}

/// The light's colour: warm white, or what `VICTORY_AMBIENT_TINT=r,g,b`
/// gives. With `VICTORY_AMBIENT_FILE=<file>` (diagnostics builds) the file's
/// `level r g b` is read again twice a second, to tune both by eye in a
/// running game.
fn tint() -> [f32; 3] {
    let live = LIVE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, tint)) = *live {
        return tint;
    }
    static SET: OnceLock<[f32; 3]> = OnceLock::new();
    *SET.get_or_init(|| {
        let set: Vec<f32> = std::env::var("VICTORY_AMBIENT_TINT")
            .unwrap_or_default()
            .split(',')
            .filter_map(|v| v.trim().parse().ok())
            .collect();
        match set[..] {
            [r, g, b] => [r, g, b],
            _ => [1.0, 0.98, 0.94],
        }
    })
}

/// The level and colour last read from `VICTORY_AMBIENT_FILE`, if any.
static LIVE: std::sync::Mutex<Option<(f32, [f32; 3])>> = std::sync::Mutex::new(None);

/// Where the resolve reads the scene's depth (reversed, D32 with stencil):
/// root parameter and place in its table.
pub(crate) const DEPTH: (u8, u32) = (0, 464);

/// How much less of the light reaches creases and corners, worked out from
/// the depth image alone: strength (0 for none), the radius looked at in
/// metres, the near plane's distance, and the tangent of half the vertical
/// field of view (the last two are guesses the picture is not sensitive
/// to). `VICTORY_AMBIENT_SHADE=s,r,n,t` sets them; the file's values after
/// its first four replace them.
/// shortcut: stands in for the occlusion the title's traced probes carry.
static SHADE: std::sync::Mutex<Option<[f32; 4]>> = std::sync::Mutex::new(None);

fn shade() -> [f32; 4] {
    if let Some(live) = *SHADE.lock().unwrap_or_else(|e| e.into_inner()) {
        return live;
    }
    static SET: OnceLock<[f32; 4]> = OnceLock::new();
    *SET.get_or_init(|| {
        let mut shade = [1.0, 0.6, 0.1, 0.45];
        let set = std::env::var("VICTORY_AMBIENT_SHADE").unwrap_or_default();
        for (to, v) in shade.iter_mut().zip(set.split(',')) {
            if let Ok(v) = v.trim().parse() {
                *to = v;
            }
        }
        shade
    })
}

/// The level from `VICTORY_AMBIENT_FILE`, if it gave one; starts the thread
/// that reads the file.
fn live_level() -> Option<f32> {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        if !cfg!(feature = "diagnostics") {
            return;
        }
        let Some(file) = std::env::var_os("VICTORY_AMBIENT_FILE") else { return };
        let _ = std::thread::Builder::new().name("victory-ambient".into()).spawn(move || {
            loop {
                let read: Vec<f32> = std::fs::read_to_string(&file)
                    .unwrap_or_default()
                    .split_whitespace()
                    .filter_map(|v| v.parse().ok())
                    .collect();
                if let [level, r, g, b, ref rest @ ..] = read[..] {
                    if let [s, r, n, t] = rest[..] {
                        *SHADE.lock().unwrap_or_else(|e| e.into_inner()) = Some([s, r, n, t]);
                    }
                    let new = Some((level.max(0.0), [r, g, b]));
                    let mut live = LIVE.lock().unwrap_or_else(|e| e.into_inner());
                    if *live != new {
                        tracing::info!(level, r, g, b, "ambient light set from the file");
                        *live = new;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        });
    });
    LIVE.lock().unwrap_or_else(|e| e.into_inner()).map(|(level, _)| level)
}

const SOURCE: &str = r#"
#define RS "RootConstants(num32BitConstants=12, b0), DescriptorTable(UAV(u0), SRV(t0), SRV(t1), UAV(u1))"
cbuffer Floor : register(b0) {
    float level; float3 tint;
    float strength; float radius; float near; float tan_y;
    float follow; float least; float steep; float spare;
};
RWTexture2D<uint> light : register(u0);
Texture2D<float> depth : register(t0);
// Try 6.6: the lit scene the resolve reads, and what the card keeps of its
// brightness from frame to frame (adapted log2 brightness, the high mark,
// frames, the factor on the light).
Texture2D<float4> scene : register(t1);
RWByteAddressBuffer state : register(u1);
groupshared float sums[64];
groupshared float counts[64];
// The eye's adaptation, on the card alone (no read back, no wait): the
// scene's mean log brightness from 256 points, eased in over frames, and
// against the brightest it has been lately. The darker the scene than that
// (night, indoors), the less of the constant light: the title raises its
// exposure there, which made people glow.
[RootSignature(RS)]
[numthreads(64, 1, 1)]
void measure(uint3 id : SV_GroupThreadID) {
    uint w, h;
    scene.GetDimensions(w, h);
    float sum = 0.0, n = 0.0;
    for (uint k = 0; k < 4; k++) {
        uint i = id.x * 4 + k;
        float2 uv = (float2(i % 16, i / 16) + 0.5) / 16.0;
        float3 c = scene.Load(int3(int2(uv * float2(w, h)), 0)).rgb;
        float l = dot(c, float3(0.2126, 0.7152, 0.0722));
        if (!isnan(l) && !isinf(l)) {
            sum += log2(clamp(l, 1e-6, 65504.0));
            n += 1.0;
        }
    }
    sums[id.x] = sum;
    counts[id.x] = n;
    GroupMemoryBarrierWithGroupSync();
    for (uint s = 32; s > 0; s >>= 1) {
        if (id.x < s) {
            sums[id.x] += sums[id.x + s];
            counts[id.x] += counts[id.x + s];
        }
        GroupMemoryBarrierWithGroupSync();
    }
    if (id.x == 0) {
        float now = counts[0] > 0.0 ? sums[0] / counts[0] : 0.0;
        float adapted = asfloat(state.Load(0));
        float high = asfloat(state.Load(4));
        uint frames = state.Load(8);
        if (frames == 0 || isnan(adapted) || isnan(high) || isinf(adapted) || isinf(high)) {
            adapted = now;
            high = now;
        }
        adapted = lerp(adapted, now, 0.1);
        high = max(high - 0.004, adapted);
        float factor = max(least, exp2(-max(high - adapted, 0.0) * steep));
        state.Store(0, asuint(adapted));
        state.Store(4, asuint(high));
        state.Store(8, frames + 1);
        state.Store(12, asuint(factor));
    }
}
// The point seen at a texel, in metres from the eye (x right, y up, z away).
float3 seen(int2 p, float2 size) {
    float d = depth.Load(int3(clamp(p, int2(0, 0), int2(size) - 1), 0));
    float z = near / max(d, 1e-7);
    float2 ndc = (float2(p) + 0.5) / size * 2.0 - 1.0;
    return float3(ndc.x * tan_y * size.x / size.y * z, -ndc.y * tan_y * z, z);
}
// How much of the light from around reaches the texel: 1 in the open.
float open_to_light(int2 p, float2 size) {
    float3 c = seen(p, size);
    if (c.z > 400.0) return 1.0;
    float3 l = seen(p + int2(-1, 0), size), r = seen(p + int2(1, 0), size);
    float3 u = seen(p + int2(0, -1), size), d = seen(p + int2(0, 1), size);
    float3 dx = abs(l.z - c.z) < abs(r.z - c.z) ? c - l : r - c;
    float3 dy = abs(u.z - c.z) < abs(d.z - c.z) ? c - u : d - c;
    float3 n = normalize(cross(dy, dx));
    if (dot(n, c) > 0.0) n = -n;
    float px = radius * size.y / (2.0 * tan_y * c.z);
    if (px < 1.5) return 1.0;
    px = min(px, 128.0);
    float noise = frac(52.9829189 * frac(dot(float2(p), float2(0.06711056, 0.00583715))));
    float sum = 0.0;
    const int N = 16;
    for (int i = 0; i < N; i++) {
        float a = i * 2.399963 + noise * 6.2831853;
        float2 o = float2(cos(a), sin(a)) * (sqrt((i + 0.5) / N) * px);
        float3 v = seen(p + int2(round(o)), size) - c;
        float vv = dot(v, v);
        float over = dot(v, n) - 0.004 * c.z - 0.005;
        sum += max(over, 0.0) / sqrt(vv + 1e-6) * saturate(1.0 - vv / (radius * radius));
    }
    return saturate(1.0 - strength * 2.0 * sum / N);
}
float3 unpack(uint v) {
    return float3(v & 511, (v >> 9) & 511, (v >> 18) & 511) * exp2(float(v >> 27) - 24.0);
}
uint pack(float3 c) {
    c = clamp(c, 0.0, 65408.0);
    float most = max(c.r, max(c.g, c.b));
    int e = clamp(int(floor(log2(max(most, 1e-9)))) + 16, 0, 31);
    float scale = exp2(float(24 - e));
    if (most * scale >= 511.5) { e = min(e + 1, 31); scale = exp2(float(24 - e)); }
    uint3 m = min(uint3(c * scale + 0.5), 511u);
    return m.r | (m.g << 9) | (m.b << 18) | (uint(e) << 27);
}
[RootSignature(RS)]
[numthreads(8, 8, 1)]
void main(uint3 at : SV_DispatchThreadID) {
    uint w, h;
    light.GetDimensions(w, h);
    if (at.x >= w || at.y >= h) return;
    // What the resolve left is not kept: without rays its inputs are never
    // written and it holds whatever their memory did (bright magenta on
    // translucent surfaces, blocks of colours indoors: run v6, 2026-11-25).
    float open = strength > 0.0 ? open_to_light(int2(at.xy), float2(w, h)) : 1.0;
    float f = follow > 0.5 ? asfloat(state.Load(12)) : 1.0;
    if (!(f > 0.0) || f > 1.0) f = 1.0;
    light[at.xy] = pack(tint * level * f * open);
}
"#;

/// Views, used in turn: more than the passes of every list in flight.
const SLOTS: u32 = 1024;

struct Pass {
    root: ID3D12RootSignature,
    pso: ID3D12PipelineState,
    /// Try 6.6: the brightness measure ([`follow`]) and its state.
    measure: ID3D12PipelineState,
    state: ID3D12Resource,
    views: ID3D12DescriptorHeap,
    step: u32,
    next: AtomicU32,
}
// SAFETY: D3D12 objects are free-threaded.
unsafe impl Send for Pass {}
// SAFETY: as above.
unsafe impl Sync for Pass {}

/// Try 6.6, the fill light follows the scene (`VICTORY_AMBIENT_FOLLOW=0`:
/// the same everywhere, as before): the least share of it in the darkest
/// scenes (`VICTORY_AMBIENT_FOLLOW_MIN`, 0.35) and how fast it falls per
/// stop the scene is darker than its brightest lately
/// (`VICTORY_AMBIENT_FOLLOW_STEEP`, 0.35). Every modern renderer scales
/// its indirect light with the scene's; this one is measured on the card
/// from the lit scene the resolve reads, with no read back.
fn follow() -> Option<(f32, f32)> {
    static SET: OnceLock<Option<(f32, f32)>> = OnceLock::new();
    *SET.get_or_init(|| {
        if std::env::var_os("VICTORY_AMBIENT_FOLLOW").is_some_and(|v| v == "0") {
            return None;
        }
        let get = |name: &str, default: f32| {
            std::env::var(name).ok().and_then(|v| v.trim().parse::<f32>().ok()).filter(|v| v.is_finite()).unwrap_or(default)
        };
        Some((get("VICTORY_AMBIENT_FOLLOW_MIN", 0.35).clamp(0.0, 1.0), get("VICTORY_AMBIENT_FOLLOW_STEEP", 0.35).clamp(0.0, 4.0)))
    })
}

/// The view format the brightness measure reads a resource of `format`
/// with, if it is a lit scene's (a floating-point colour image).
fn scene_format(format: DXGI_FORMAT) -> Option<DXGI_FORMAT> {
    match format {
        DXGI_FORMAT_R11G11B10_FLOAT => Some(DXGI_FORMAT_R11G11B10_FLOAT),
        DXGI_FORMAT_R16G16B16A16_FLOAT | DXGI_FORMAT_R16G16B16A16_TYPELESS => Some(DXGI_FORMAT_R16G16B16A16_FLOAT),
        DXGI_FORMAT_R32G32B32A32_FLOAT | DXGI_FORMAT_R32G32B32A32_TYPELESS => Some(DXGI_FORMAT_R32G32B32A32_FLOAT),
        DXGI_FORMAT_R9G9B9E5_SHAREDEXP => Some(DXGI_FORMAT_R9G9B9E5_SHAREDEXP),
        _ => None,
    }
}

fn compile(entry: windows::core::PCSTR) -> windows::core::Result<Vec<u8>> {
    let mut code: Option<ID3DBlob> = None;
    let mut errors: Option<ID3DBlob> = None;
    // SAFETY: the source and names live through the call; out pointers ours.
    let compiled = unsafe {
        D3DCompile(
            SOURCE.as_ptr().cast(),
            SOURCE.len(),
            s!("ambient"),
            None,
            None,
            entry,
            s!("cs_5_1"),
            0,
            0,
            &mut code,
            Some(&mut errors),
        )
    };
    let text = |b: &ID3DBlob| {
        // SAFETY: the blob's own bytes.
        unsafe { std::slice::from_raw_parts(b.GetBufferPointer().cast::<u8>(), b.GetBufferSize()) }
            .to_vec()
    };
    if let Err(e) = compiled {
        if let Some(errors) = errors {
            tracing::warn!("ambient light: {}", String::from_utf8_lossy(&text(&errors)));
        }
        return Err(e);
    }
    Ok(text(&code.ok_or_else(windows::core::Error::empty)?))
}

fn make(device: &ID3D12Device) -> windows::core::Result<Pass> {
    let code = compile(s!("main"))?;
    let measure_code = compile(s!("measure"))?;
    // SAFETY: the root signature is inside the shader; plain creations.
    unsafe {
        let root: ID3D12RootSignature = device.CreateRootSignature(0, &code)?;
        let measure = device.CreateComputePipelineState(&D3D12_COMPUTE_PIPELINE_STATE_DESC {
            pRootSignature: std::mem::transmute_copy(&root),
            CS: D3D12_SHADER_BYTECODE {
                pShaderBytecode: measure_code.as_ptr().cast(),
                BytecodeLength: measure_code.len(),
            },
            ..Default::default()
        })?;
        let heap = D3D12_HEAP_PROPERTIES {
            Type: D3D12_HEAP_TYPE_DEFAULT,
            ..Default::default()
        };
        let buffer = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
            Alignment: 0,
            Width: 256,
            Height: 1,
            DepthOrArraySize: 1,
            MipLevels: 1,
            Format: DXGI_FORMAT_UNKNOWN,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
            Flags: D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS,
        };
        let mut state: Option<ID3D12Resource> = None;
        device.CreateCommittedResource(&heap, D3D12_HEAP_FLAG_NONE, &buffer, D3D12_RESOURCE_STATE_COMMON, None, &mut state)?;
        let state = state.ok_or_else(windows::core::Error::empty)?;
        let pso = device.CreateComputePipelineState(&D3D12_COMPUTE_PIPELINE_STATE_DESC {
            pRootSignature: std::mem::transmute_copy(&root),
            CS: D3D12_SHADER_BYTECODE {
                pShaderBytecode: code.as_ptr().cast(),
                BytecodeLength: code.len(),
            },
            ..Default::default()
        })?;
        let kind = D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV;
        let views = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
            Type: kind,
            NumDescriptors: SLOTS * 4,
            Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
            NodeMask: 0,
        })?;
        Ok(Pass {
            root,
            pso,
            measure,
            state,
            views,
            step: device.GetDescriptorHandleIncrementSize(kind),
            next: AtomicU32::new(0),
        })
    }
}

fn uav_barrier(list: &ID3D12GraphicsCommandList) {
    let barrier = D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_UAV,
        ..Default::default()
    };
    // SAFETY: an open list; the barrier names no resource.
    unsafe { list.ResourceBarrier(&[barrier]) };
}

/// Raises what `light` holds to the floor, in `list`, after the program that
/// wrote it as an image. Whether it recorded anything (the list's bindings
/// are then ours).
pub(crate) fn raise(list: &HostList, light: This, depth: Option<This>, scenes: &[This]) -> bool {
    let (Some(level), Some(host)) = (level(), host()) else {
        return false;
    };
    let Some(light) = crate::resource::host_resource(light) else {
        return false;
    };
    static PASS: OnceLock<Option<Pass>> = OnceLock::new();
    let Some(pass) = PASS
        .get_or_init(|| {
            make(&host.device)
                .inspect_err(|e| tracing::warn!(error = %e, "no ambient light"))
                .ok()
        })
        .as_ref()
    else {
        return false;
    };
    // SAFETY: a live resource.
    let d = unsafe { light.GetDesc() };
    static ONCE: std::sync::Once = std::sync::Once::new();
    if d.Format != DXGI_FORMAT_R32_UINT {
        ONCE.call_once(|| tracing::warn!(format = d.Format.0, "no ambient light: another format"));
        return false;
    }
    ONCE.call_once(|| {
        tracing::info!(level, width = d.Width, height = d.Height, "ambient light");
    });
    let slot = pass.next.fetch_add(1, Ordering::Relaxed) % SLOTS * 4;
    // The lit scene among what the resolve reads: the first colour image
    // in floating point other than the light itself ([`follow`]).
    let scene = follow().and_then(|_| {
        scenes.iter().filter_map(|&t| crate::resource::host_resource(t)).find_map(|t| {
            // SAFETY: a live resource.
            let s = unsafe { t.GetDesc() };
            let format = scene_format(s.Format)?;
            (s.Dimension == D3D12_RESOURCE_DIMENSION_TEXTURE2D && s.Width >= 64 && s.Height >= 64 && t != light)
                .then_some((t, format, s.Width, s.Height, s.Format.0))
        })
    });
    static SCENE_ONCE: std::sync::Once = std::sync::Once::new();
    SCENE_ONCE.call_once(|| match scene {
        Some((_, _, width, height, format)) => tracing::info!(
            width,
            height,
            format,
            "ambient light follows the scene's brightness, measured on the card (VICTORY_AMBIENT_FOLLOW=0: the same everywhere)"
        ),
        None => tracing::info!(
            candidates = scenes.len(),
            "ambient light: no lit scene among what the resolve reads, the same everywhere"
        ),
    });
    if let Some((t, ..)) = scene {
        list.keep(t);
    }
    list.keep(light);
    // The depth the resolve just read, in the state it read it in; of the
    // light's size, or the shading is left out.
    let depth = depth.and_then(crate::resource::host_resource).filter(|t| {
        // SAFETY: a live resource.
        let t = unsafe { t.GetDesc() };
        t.Format == DXGI_FORMAT_R32G8X24_TYPELESS && (t.Width, t.Height) == (d.Width, d.Height)
    });
    if let Some(depth) = depth {
        list.keep(depth);
    }
    let l = &list.list;
    uav_barrier(l);
    // SAFETY: an open list; a live texture the list keeps, in the
    // unordered-access state as the program before left it; a view in a slot
    // no list in flight reads.
    unsafe {
        let cpu = pass.views.GetCPUDescriptorHandleForHeapStart();
        let gpu = pass.views.GetGPUDescriptorHandleForHeapStart();
        let at = D3D12_CPU_DESCRIPTOR_HANDLE {
            ptr: cpu.ptr + (slot * pass.step) as usize,
        };
        let view = D3D12_UNORDERED_ACCESS_VIEW_DESC {
            Format: DXGI_FORMAT_R32_UINT,
            ViewDimension: D3D12_UAV_DIMENSION_TEXTURE2D,
            ..Default::default()
        };
        host.device
            .CreateUnorderedAccessView(light, None::<&ID3D12Resource>, Some(&view), at);
        let seen = D3D12_SHADER_RESOURCE_VIEW_DESC {
            Format: DXGI_FORMAT_R32_FLOAT_X8X24_TYPELESS,
            ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
            Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
            Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                Texture2D: D3D12_TEX2D_SRV { MipLevels: 1, ..Default::default() },
            },
        };
        let beside = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: at.ptr + pass.step as usize };
        host.device.CreateShaderResourceView(depth, Some(&seen), beside);
        let lit = D3D12_SHADER_RESOURCE_VIEW_DESC {
            Format: scene.map_or(DXGI_FORMAT_R16G16B16A16_FLOAT, |s| s.1),
            ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
            Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
            Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                Texture2D: D3D12_TEX2D_SRV { MipLevels: 1, ..Default::default() },
            },
        };
        let third = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: at.ptr + 2 * pass.step as usize };
        host.device.CreateShaderResourceView(scene.map(|s| s.0), Some(&lit), third);
        let kept = D3D12_UNORDERED_ACCESS_VIEW_DESC {
            Format: DXGI_FORMAT_R32_TYPELESS,
            ViewDimension: D3D12_UAV_DIMENSION_BUFFER,
            Anonymous: D3D12_UNORDERED_ACCESS_VIEW_DESC_0 {
                Buffer: D3D12_BUFFER_UAV {
                    FirstElement: 0,
                    NumElements: 64,
                    StructureByteStride: 0,
                    CounterOffsetInBytes: 0,
                    Flags: D3D12_BUFFER_UAV_FLAG_RAW,
                },
            },
        };
        let fourth = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: at.ptr + 3 * pass.step as usize };
        host.device
            .CreateUnorderedAccessView(&pass.state, None::<&ID3D12Resource>, Some(&kept), fourth);
        l.SetDescriptorHeaps(&[Some(pass.views.clone())]);
        l.SetComputeRootSignature(&pass.root);
        l.SetPipelineState(&pass.pso);
        let [r, g, b] = tint();
        let level = live_level().unwrap_or(level);
        let [strength, radius, near, tan_y] = shade();
        let strength = if depth.is_some() { strength } else { 0.0 };
        let (least, steep) = follow().unwrap_or((1.0, 0.0));
        let following = if scene.is_some() { 1.0 } else { 0.0 };
        let constants = [level, r, g, b, strength, radius, near, tan_y, following, least, steep, 0.0];
        l.SetComputeRoot32BitConstants(0, 12, constants.as_ptr().cast(), 0);
        l.SetComputeRootDescriptorTable(
            1,
            D3D12_GPU_DESCRIPTOR_HANDLE {
                ptr: gpu.ptr + u64::from(slot * pass.step),
            },
        );
        if scene.is_some() {
            l.SetPipelineState(&pass.measure);
            l.Dispatch(1, 1, 1);
            uav_barrier(l);
            l.SetPipelineState(&pass.pso);
        }
        l.Dispatch((d.Width as u32).div_ceil(8), d.Height.div_ceil(8), 1);
    }
    uav_barrier(l);
    true
}

#[cfg(test)]
mod tests {
    #[test]
    fn roles_by_name() {
        use super::{Role, role};
        assert_eq!(role(b"..CS_ScreenProbes_TemporalResolve_a911b49c_Wrapped.."), Some(Role::Resolve));
        assert_eq!(role(b"..CS_CacheScreenSpaceRadiance_eed2fc35.."), Some(Role::Radiance));
        assert_eq!(role(b"CS_AccumulateLocalLightInscatteringVolume_7e2f5eef"), Some(Role::Air));
        assert_eq!(role(b"CS_TemporalResolve"), None);
        assert_eq!(role(b"..CS_RayMiss_de6b91a6_Wrapped.."), Some(Role::Miss));
        assert_eq!(role(b"..CS_RayMiss_Volumetrics_3c5d8c00_Wrapped.."), None);
        assert_eq!(role(b"..CS_ScreenProbe_RayMiss_80a88544_Wrapped.."), None);
    }

    /// The pass compiles with the system's compiler and the host takes it.
    #[test]
    fn ambient_pass_builds() {
        let Some(host) = crate::host::host() else {
            return;
        };
        super::make(&host.device).expect("the ambient pass");
    }
}
