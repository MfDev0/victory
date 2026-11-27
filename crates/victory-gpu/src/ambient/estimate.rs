//! The world's indirect light, reflections and air, estimated from the frame
//! (Try 6.25: trustno1-coder/VIctory-NVIDIA's `ambient.rs`, from foxhound,
//! GPL-3.0-or-later; the shaders are theirs unchanged, the binding is this
//! fork's own descriptor ring as [`super::raise`] has it).
//!
//! With the title's rays finding nothing, its resolve of the screen probes
//! leaves the sky's light outdoors and black indoors. Instead of one constant
//! everywhere ([`super::raise`], this fork's fill light, which cutscenes keep:
//! `crate::cutscene`), the light the visible surfaces send out (the title keeps
//! it for its own screen traces: `CS_CacheScreenSpaceRadiance`, the frame
//! before) is averaged in 64x36 cells, and every point gets the mean of the
//! cells weighted by the share of its surroundings they cover, as a
//! directional value (first-order harmonics at 160x90 points, taken between
//! them by depth). It follows the hour, the room and its lamps by itself.
//!
//! - `VICTORY_AMBIENT_ESTIMATE=<share>`: that share of the estimate (0.8, as
//!   VIctory-NVIDIA runs it; `0` or `off`: this fork's constant everywhere, as
//!   before 6.25). `VICTORY_AMBIENT=off` leaves the title's own light.
//! - `VICTORY_AMBIENT_AIR=<multiple>` (1.5): what the air of a room scatters
//!   is bounded by a multiple of the light around (without rays the probes
//!   hold the sky's light indoors too: a brown veil at sunset). `0`: off.
//! - `VICTORY_AMBIENT_SPECULAR=<share>[,<sky>]` (1): reflection rays that
//!   found nothing (`CS_RayMiss`) take the estimate along the reflected
//!   direction instead of the sky's light; `sky` of the title's sky is kept,
//!   made no brighter. `0`: the title's sky. `VICTORY_AMBIENT_SPECULAR_ALPHA=1`
//!   does it for the alpha layer too (glass, foam; dark patches on water).
//! - `VICTORY_AMBIENT_ONCE=1`: one bounce of the title's own light, without
//!   the estimate's light coming back to it.
//!
//! A camera cut of the title's (`crate::cutscene`) starts the slow mean again
//! from that frame.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D::ID3DBlob;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::{PCSTR, s};

use super::Role;
use crate::host::{HostList, host};

/// The share of the estimate; `None`: off.
pub(crate) fn share() -> Option<f32> {
    static SET: OnceLock<Option<f32>> = OnceLock::new();
    *SET.get_or_init(|| {
        if std::env::var("VICTORY_AMBIENT").is_ok_and(|v| v.trim() == "off") {
            return None;
        }
        parse_share(std::env::var("VICTORY_AMBIENT_ESTIMATE").ok().as_deref())
    })
}

fn parse_share(set: Option<&str>) -> Option<f32> {
    match set.map(str::trim) {
        None | Some("") => Some(0.8),
        Some(v) => v.parse::<f32>().ok().filter(|v| v.is_finite() && *v > 0.0),
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Specular {
    share: f32,
    sky: f32,
}

fn parse_specular(set: Option<&str>) -> Option<Specular> {
    let set = match set.map(str::trim) {
        None | Some("") => "1",
        Some(v) => v,
    };
    let mut parts = set.split(',').map(|t| t.trim().parse::<f32>().ok().filter(|v| v.is_finite()));
    let share = parts.next().flatten().filter(|v| *v > 0.0)?;
    let sky = parts.next().flatten().map_or(0.0, |v| v.clamp(0.0, 1.0));
    Some(Specular { share, sky })
}

pub(crate) fn specular() -> Option<Specular> {
    static SET: OnceLock<Option<Specular>> = OnceLock::new();
    *SET.get_or_init(|| {
        share()?;
        parse_specular(std::env::var("VICTORY_AMBIENT_SPECULAR").ok().as_deref())
    })
}

fn parse_air(set: Option<&str>) -> f32 {
    match set.map(str::trim) {
        None | Some("") => 1.5,
        Some(v) => v.parse().ok().filter(|v: &f32| v.is_finite() && *v > 0.0).unwrap_or(0.0),
    }
}

fn air() -> f32 {
    static SET: OnceLock<f32> = OnceLock::new();
    *SET.get_or_init(|| parse_air(std::env::var("VICTORY_AMBIENT_AIR").ok().as_deref()))
}

fn once() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("VICTORY_AMBIENT_ONCE").is_some_and(|v| v == "1"))
}

/// Whether reflections of the alpha layer get it too.
pub(crate) fn alpha_too() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("VICTORY_AMBIENT_SPECULAR_ALPHA").is_ok_and(|v| v.trim() == "1"))
}

/// Whether a dispatch of a program of `role` gets a pass now: the estimate
/// is on, and (but for the keep, which the estimate needs ready when a
/// cutscene ends) not in a cutscene.
pub(crate) fn wanted(role: Role) -> bool {
    if share().is_none() || crate::pipeline::builder_runs() {
        return false;
    }
    match role {
        Role::Radiance => true,
        Role::Resolve => !crate::cutscene::now(),
        Role::Air => air() > 0.0 && !crate::cutscene::now(),
        Role::Miss => specular().is_some() && !crate::cutscene::now(),
    }
}

const SOURCE: &str = r#"
#define RS "RootConstants(num32BitConstants=12, b0), DescriptorTable(SRV(t0)), DescriptorTable(UAV(u0)), UAV(u1), UAV(u2)"
#define CX 64
#define CY 36
#define CELLS (CX * CY)
#define PX 160
#define PY 90
cbuffer Set : register(b0) {
    float level; float constant; float strength; float radius;
    float near; float tan_y; uint width; uint height;
    float follow; float most; float air; float once;
};
Texture2D<float> depth : register(t0);
// Cells: the light they send out (then, at CELLS, the slow mean), from
// CELLS + 1 on where they are, from SHARES on the share of the title's own
// light at each of the 25 places of a cell the keep reads (x; y: written).
#define SHARES (2 * CELLS + 1)
RWStructuredBuffer<float4> cells : register(u1);
// Three a point: red, green and blue, each a mean and a direction.
RWStructuredBuffer<float4> points : register(u2);
static const float3 LUMA = float3(0.2126, 0.7152, 0.0722);

// The point seen at a texel, in metres from the eye (x right, y up, z away).
float3 seen(int2 p, float2 size) {
    float d = depth.Load(int3(clamp(p, int2(0, 0), int2(size) - 1), 0));
    float z = min(near / max(d, 1e-7), 1000.0);
    float2 ndc = (float2(p) + 0.5) / size * 2.0 - 1.0;
    return float3(ndc.x * tan_y * size.x / size.y * z, -ndc.y * tan_y * z, z);
}

#ifdef KEEP
RWTexture2D<float3> radiance : register(u0);
[RootSignature(RS)]
[numthreads(8, 4, 1)]
void keep(uint3 at : SV_DispatchThreadID) {
    uint w, h;
    radiance.GetDimensions(w, h);
    float3 sum = 0;
    for (uint y = 0; y < 5; y++) for (uint x = 0; x < 5; x++) {
        uint2 p = uint2((float2(at.xy) + (float2(x, y) + 0.5) / 5.0) * float2(w, h) / float2(CX, CY));
        float3 c = max(radiance[p], 0.0);
        c = any(isnan(c)) || any(isinf(c)) ? 0.0 : c;
        // Lamps and glints count, but not for everything.
        c /= max(1.0, dot(c, LUMA) / 20.0);
        // Not the stand-in's own light, reflected (`once`).
        float4 share = cells[SHARES + (at.y * CX + at.x) * 25 + y * 5 + x];
        sum += once > 0.0 && share.y > 0.0 ? c * saturate(share.x) : c;
    }
    cells[at.y * CX + at.x] = float4(sum / 25.0, 1.0);
}
[RootSignature(RS)]
[numthreads(1, 1, 1)]
void mean() {
    float3 sum = 0;
    for (uint i = 0; i < CELLS; i++) sum += cells[i].rgb;
    float4 was = cells[CELLS];
    float3 now = sum / CELLS;
    cells[CELLS] = float4(was.a > 0.0 ? lerp(was.rgb, now, follow) : now, 1.0);
}
#endif

#ifdef PLACE
[RootSignature(RS)]
[numthreads(8, 4, 1)]
void place(uint3 at : SV_DispatchThreadID) {
    float2 size = float2(width, height);
    float3 p = seen(int2((float2(at.xy) + 0.5) * size / float2(CX, CY)), size);
    cells[CELLS + 1 + at.y * CX + at.x] = float4(p, p.z * p.z);
}
#endif

#ifdef GATHER
[RootSignature(RS)]
[numthreads(8, 2, 1)]
void gather(uint3 at : SV_DispatchThreadID) {
    float2 size = float2(width, height);
    float3 p = seen(int2((float2(at.xy) + 0.5) * size / float2(PX, PY)), size);
    float r2 = 0.25 + 0.0025 * p.z * p.z;
    float whole = 0;
    float3 toward = 0, light = 0, red = 0, green = 0, blue = 0;
    for (uint i = 0; i < CELLS; i++) {
        float4 c = cells[CELLS + 1 + i];
        float3 L = cells[i].rgb;
        float3 d = c.xyz - p;
        float dd = dot(d, d);
        float w = c.w / (dd + r2);
        float3 dir = d / sqrt(dd + 0.25 * r2);
        whole += w;
        toward += w * dir;
        light += w * L;
        red += w * L.r * dir;
        green += w * L.g * dir;
        blue += w * L.b * dir;
    }
    whole = max(whole, 1e-6);
    light /= whole;
    toward /= whole;
    uint at0 = (at.y * PX + at.x) * 3;
    points[at0] = float4(red / whole - light.r * toward, light.r);
    points[at0 + 1] = float4(green / whole - light.g * toward, light.g);
    points[at0 + 2] = float4(blue / whole - light.b * toward, light.b);
}
#endif

#ifdef AIR
RWTexture3D<float3> scattered : register(u0);
// No more than a multiple of the light around: of the room as the last
// seconds showed it, and of the surface seen in this direction (towards a
// window the air outside may be as bright as the sky is).
[RootSignature(RS)]
[numthreads(8, 8, 2)]
void clear_air(uint3 at : SV_DispatchThreadID) {
    uint w, h, d;
    scattered.GetDimensions(w, h, d);
    if (at.x >= w || at.y >= h || at.z >= d) return;
    uint2 q = min(uint2((float2(at.xy) + 0.5) / float2(w, h) * float2(PX, PY)), uint2(PX - 1, PY - 1));
    uint at0 = (q.y * PX + q.x) * 3;
    float3 seen_there = float3(points[at0].w, points[at0 + 1].w, points[at0 + 2].w);
    float far = float(at.z) / float(d);
    float cap = air * lerp(dot(cells[CELLS].rgb, LUMA), dot(seen_there, LUMA), far);
    float3 c = scattered[at];
    float has = dot(c, LUMA);
    if (has > cap) scattered[at] = c * (cap / has);
}
#endif

#ifdef REFLECT
RWTexture2D<float3> reflected : register(u0);
// Each texel of the reflections' image (half the picture's size): the light
// the points around send along the reflected direction, as `raise` takes it
// along the normal; `level` is the share, `constant` the share of the
// title's sky kept, made no brighter than that.
[RootSignature(RS)]
[numthreads(8, 8, 1)]
void reflect_light(uint3 at : SV_DispatchThreadID) {
    uint w, h;
    reflected.GetDimensions(w, h);
    if (at.x >= w || at.y >= h) return;
    float2 size = float2(width, height);
    int2 p = int2((float2(at.xy) + 0.5) * size / float2(w, h));
    float3 c = seen(p, size);
    // Nothing is reflected by the sky.
    if (c.z >= 999.0) return;
    float3 l = seen(p + int2(-1, 0), size), r = seen(p + int2(1, 0), size);
    float3 u = seen(p + int2(0, -1), size), d = seen(p + int2(0, 1), size);
    float3 dx = abs(l.z - c.z) < abs(r.z - c.z) ? c - l : r - c;
    float3 dy = abs(u.z - c.z) < abs(d.z - c.z) ? c - u : d - c;
    float3 n = normalize(cross(dy, dx) + float3(0, 0, -1e-9));
    if (dot(n, c) > 0.0) n = -n;
    float3 v = normalize(c);
    float3 ray = v - 2.0 * dot(v, n) * n;
    float2 g = (float2(p) + 0.5) / size * float2(PX, PY) - 0.5;
    int2 g0 = int2(floor(g));
    float2 f = g - float2(g0);
    float4 sr = 0, sg = 0, sb = 0;
    float sum = 0;
    for (int k = 0; k < 4; k++) {
        int2 q = clamp(g0 + int2(k & 1, k >> 1), int2(0, 0), int2(PX, PY) - 1);
        float z = seen(int2((float2(q) + 0.5) * size / float2(PX, PY)), size).z;
        float2 b = float2(k & 1 ? f.x : 1 - f.x, k >> 1 ? f.y : 1 - f.y);
        float wgt = (b.x * b.y + 1e-3) / (0.02 + abs(z - c.z) / c.z);
        uint at0 = (q.y * PX + q.x) * 3;
        sr += wgt * points[at0];
        sg += wgt * points[at0 + 1];
        sb += wgt * points[at0 + 2];
        sum += wgt;
    }
    float3 all = float3(sr.w, sg.w, sb.w) / sum;
    // Radiance from the first moments: three times the direction's share
    // for an even spread of directions, a little less for a smooth image.
    float3 found = all + 2.0 * float3(dot(sr.xyz, ray), dot(sg.xyz, ray), dot(sb.xyz, ray)) / sum;
    found = clamp(found, 0.1 * all, 3.0 * all);
    found = lerp(found, cells[CELLS].rgb, 0.2) * level;
    found = any(isnan(found)) || any(isinf(found)) ? 0.0 : max(found, 0.0);
    float3 was = reflected[at.xy];
    was = any(isnan(was)) || any(isinf(was)) ? 0.0 : max(was, 0.0);
    float has = dot(was, LUMA);
    was *= min(1.0, dot(found, LUMA) / max(has, 1e-6));
    reflected[at.xy] = lerp(found, was, constant);
}
#endif

#ifdef RAISE
RWTexture2D<uint> light : register(u0);
float3 unpack(uint v) {
    return float3(v & 511, (v >> 9) & 511, (v >> 18) & 511) * exp2(float(v >> 27) - 24.0);
}
uint pack(float3 c) {
    c = clamp(c, 0.0, 65408.0);
    float top = max(c.r, max(c.g, c.b));
    int e = clamp(int(floor(log2(max(top, 1e-9)))) + 16, 0, 31);
    float scale = exp2(float(24 - e));
    if (top * scale >= 511.5) { e = min(e + 1, 31); scale = exp2(float(24 - e)); }
    uint3 m = min(uint3(c * scale + 0.5), 511u);
    return m.r | (m.g << 9) | (m.b << 18) | (uint(e) << 27);
}
[RootSignature(RS)]
[numthreads(8, 8, 1)]
void raise(uint3 at : SV_DispatchThreadID) {
    float2 size = float2(width, height);
    if (at.x >= width || at.y >= height) return;
    int2 p = int2(at.xy);
    float3 c = seen(p, size);
    float3 l = seen(p + int2(-1, 0), size), r = seen(p + int2(1, 0), size);
    float3 u = seen(p + int2(0, -1), size), d = seen(p + int2(0, 1), size);
    float3 dx = abs(l.z - c.z) < abs(r.z - c.z) ? c - l : r - c;
    float3 dy = abs(u.z - c.z) < abs(d.z - c.z) ? c - u : d - c;
    float3 n = normalize(cross(dy, dx) + float3(0, 0, -1e-9));
    if (dot(n, c) > 0.0) n = -n;
    // How open it is (as in beloooo's cd9f78ce).
    float open = 1.0;
    float px = min(radius * size.y / (2.0 * tan_y * c.z), 128.0);
    float noise = frac(52.9829189 * frac(dot(float2(p), float2(0.06711056, 0.00583715))));
    if (strength > 0.0 && px >= 1.5 && c.z < 400.0) {
        float sum = 0.0;
        const int N = 12;
        for (int i = 0; i < N; i++) {
            float a = i * 2.399963 + noise * 6.2831853;
            float2 o = float2(cos(a), sin(a)) * (sqrt((i + 0.5) / N) * px);
            float3 v = seen(p + int2(round(o)), size) - c;
            float vv = dot(v, v);
            float over = dot(v, n) - 0.004 * c.z - 0.005;
            sum += max(over, 0.0) / sqrt(vv + 1e-6) * saturate(1.0 - vv / (radius * radius));
        }
        open = saturate(1.0 - strength * 2.0 * sum / N);
    }
    float3 found;
    if (constant > 0.0) {
        found = float3(1.0, 0.98, 0.94) * constant * open;
        light[p] = pack(found);
        return;
    }
    // Between the four points around, those at its own depth first; which
    // four is dithered a little.
    float2 g = (float2(p) + 0.5) / size * float2(PX, PY) - 0.5 + (noise - 0.5) * 0.5;
    int2 g0 = int2(floor(g));
    float2 f = g - float2(g0);
    float4 sr = 0, sg = 0, sb = 0;
    float sum = 0;
    for (int k = 0; k < 4; k++) {
        int2 q = clamp(g0 + int2(k & 1, k >> 1), int2(0, 0), int2(PX, PY) - 1);
        float z = seen(int2((float2(q) + 0.5) * size / float2(PX, PY)), size).z;
        float2 b = float2(k & 1 ? f.x : 1 - f.x, k >> 1 ? f.y : 1 - f.y);
        float w = (b.x * b.y + 1e-3) / (0.02 + abs(z - c.z) / c.z);
        uint at0 = (q.y * PX + q.x) * 3;
        sr += w * points[at0];
        sg += w * points[at0 + 1];
        sb += w * points[at0 + 2];
        sum += w;
    }
    float3 all = float3(sr.w, sg.w, sb.w) / sum;
    found = all + 1.2 * float3(dot(sr.xyz, n), dot(sg.xyz, n), dot(sb.xyz, n)) / sum;
    found = clamp(found, 0.2 * all, 2.5 * all);
    found = lerp(found, cells[CELLS].rgb, 0.2) * level * open;
    float3 was = unpack(light[p]);
    // Magenta/violet garbage the probes' memory held (green far under red and
    // blue): dropped, as VIctory-Belobug-AMD does (ambient.rs, "Filter magenta
    // garbage probes"). Seen on an RTX 5080 as a purple-blue glow on a lit
    // sheet of paper in the prison cell; not an AMD-only fault.
    if (was.g < 0.5 * min(was.r, was.b) && max(was.r, was.b) > 0.02) was = 0.0;
    // And the bluer kind of it (blue over 2.5x green and over red): left as
    // a faint violet by the line above. The sky's light keeps green above
    // 0.4x blue, sunsets keep blue low, so neither is touched.
    if (was.b > 2.5 * was.g && was.b > was.r && was.b > 0.02) was = 0.0;
    // What the program left counts as far as it can be true: the sky's light
    // outdoors, not the blocks of other memory.
    float top = max(dot(found, LUMA) * most, 1e-6);
    float has = dot(was, LUMA);
    was *= min(1.0, top / max(has, 1e-6));
    float3 now = max(was, found);
    light[p] = pack(now);
    if (once > 0.0) {
        // At the places the next keep reads, the share of the light that is
        // the title's own.
        float2 gc = (float2(p) + 0.5) / size * float2(CX, CY);
        uint2 cell = min(uint2(gc), uint2(CX - 1, CY - 1));
        uint2 k = min(uint2((gc - float2(cell)) * 5.0), uint2(4, 4));
        uint2 q = uint2((float2(cell) + (float2(k) + 0.5) / 5.0) * size / float2(CX, CY));
        if (q.x == uint(p.x) && q.y == uint(p.y)) {
            float share = dot(was, LUMA) / max(dot(now, LUMA), 1e-6);
            cells[SHARES + (cell.y * CX + cell.x) * 25 + k.y * 5 + k.x] = float4(share, 1.0, 0.0, 0.0);
        }
    }
}
#endif
"#;
/// Bytes of the cells' buffer and of the points'.
const CELLS: u64 = 64 * 36;
const CELL_BYTES: u64 = (2 * CELLS + 1 + 25 * CELLS) * 16;
const POINT_BYTES: u64 = 160 * 90 * 48;
/// Pairs of views (depth, image), used in turn: more than the passes of
/// every list in flight.
const SLOTS: u32 = 1024;

struct Pass {
    root: ID3D12RootSignature,
    keep: ID3D12PipelineState,
    mean: ID3D12PipelineState,
    place: ID3D12PipelineState,
    gather: ID3D12PipelineState,
    raise: ID3D12PipelineState,
    air: ID3D12PipelineState,
    reflect: ID3D12PipelineState,
    cells: ID3D12Resource,
    points: ID3D12Resource,
    views: ID3D12DescriptorHeap,
    step: u32,
    next: AtomicU32,
}
// SAFETY: D3D12 objects are free-threaded.
unsafe impl Send for Pass {}
// SAFETY: as above.
unsafe impl Sync for Pass {}

fn compile(part: &str, entry: PCSTR) -> windows::core::Result<Vec<u8>> {
    let source = format!("#define {part}\n{SOURCE}");
    let mut code: Option<ID3DBlob> = None;
    let mut errors: Option<ID3DBlob> = None;
    // SAFETY: the source and names live through the call; out pointers ours.
    let compiled = unsafe {
        D3DCompile(
            source.as_ptr().cast(),
            source.len(),
            s!("ambient-estimate"),
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
    let bytes = |b: &ID3DBlob| {
        // SAFETY: the blob's own bytes.
        unsafe { std::slice::from_raw_parts(b.GetBufferPointer().cast::<u8>(), b.GetBufferSize()) }.to_vec()
    };
    if let Err(e) = compiled {
        if let Some(errors) = errors {
            tracing::warn!("light estimate ({part}): {}", String::from_utf8_lossy(&bytes(&errors)));
        }
        return Err(e);
    }
    Ok(bytes(&code.ok_or_else(windows::core::Error::empty)?))
}

fn make(device: &ID3D12Device, buffer: impl Fn(u64) -> Option<ID3D12Resource>) -> windows::core::Result<Pass> {
    let mut root: Option<ID3D12RootSignature> = None;
    let mut pipeline = |part: &str, entry| -> windows::core::Result<ID3D12PipelineState> {
        let code = compile(part, entry)?;
        // SAFETY: the root signature is inside the shader; plain creations.
        unsafe {
            if root.is_none() {
                root = Some(device.CreateRootSignature(0, &code)?);
            }
            let desc = D3D12_COMPUTE_PIPELINE_STATE_DESC {
                pRootSignature: std::mem::transmute_copy(&root),
                CS: D3D12_SHADER_BYTECODE { pShaderBytecode: code.as_ptr().cast(), BytecodeLength: code.len() },
                ..Default::default()
            };
            device.CreateComputePipelineState(&desc)
        }
    };
    let keep = pipeline("KEEP", s!("keep"))?;
    let mean = pipeline("KEEP", s!("mean"))?;
    let place = pipeline("PLACE", s!("place"))?;
    let gather = pipeline("GATHER", s!("gather"))?;
    let raise = pipeline("RAISE", s!("raise"))?;
    let air = pipeline("AIR", s!("clear_air"))?;
    let reflect = pipeline("REFLECT", s!("reflect_light"))?;
    let made = |bytes| buffer(bytes).ok_or_else(windows::core::Error::empty);
    let kind = D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV;
    // SAFETY: plain creations.
    let (views, step) = unsafe {
        let views: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
            Type: kind,
            NumDescriptors: SLOTS * 2,
            Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
            NodeMask: 0,
        })?;
        (views, device.GetDescriptorHandleIncrementSize(kind))
    };
    Ok(Pass {
        root: root.ok_or_else(windows::core::Error::empty)?,
        keep,
        mean,
        place,
        gather,
        raise,
        air,
        reflect,
        cells: made(CELL_BYTES)?,
        points: made(POINT_BYTES)?,
        views,
        step,
        next: AtomicU32::new(0),
    })
}

fn pass() -> Option<&'static Pass> {
    static PASS: OnceLock<Option<Pass>> = OnceLock::new();
    PASS.get_or_init(|| {
        let host = host()?;
        make(&host.device, |bytes| host.uav_buffer(bytes))
            .inspect_err(|e| tracing::warn!(error = %e, "no light estimate: this fork's fill light everywhere"))
            .ok()
    })
    .as_ref()
}

fn uav_barrier(list: &ID3D12GraphicsCommandList) {
    let barrier = D3D12_RESOURCE_BARRIER { Type: D3D12_RESOURCE_BARRIER_TYPE_UAV, ..Default::default() };
    // SAFETY: an open list; the barrier names no resource.
    unsafe { list.ResourceBarrier(&[barrier]) };
}

/// Whether the light of a frame has been kept: the estimate has something
/// to begin with.
static KEPT: AtomicBool = AtomicBool::new(false);

/// What a pass reads and writes: the image the title's program just wrote,
/// and the scene's depth it read (for [`Role::Resolve`] and [`Role::Miss`]).
pub(crate) struct Target {
    pub(crate) image: ID3D12Resource,
    pub(crate) depth: Option<ID3D12Resource>,
}

/// The image's format the passes take for `role`: R32_UINT holding
/// R9G9B9E5 for the resolve, R11G11B10 for the others.
pub(crate) fn image_format(role: Role) -> DXGI_FORMAT {
    if role == Role::Resolve { DXGI_FORMAT_R32_UINT } else { DXGI_FORMAT_R11G11B10_FLOAT }
}

/// The view format a 32-bit depth is read with; `None` for others (a 24-bit
/// depth is not the reversed float the passes take).
pub(crate) fn depth_view(format: DXGI_FORMAT) -> Option<DXGI_FORMAT> {
    match format {
        DXGI_FORMAT_R32G8X24_TYPELESS | DXGI_FORMAT_D32_FLOAT_S8X24_UINT => Some(DXGI_FORMAT_R32_FLOAT_X8X24_TYPELESS),
        DXGI_FORMAT_R32_TYPELESS | DXGI_FORMAT_D32_FLOAT | DXGI_FORMAT_R32_FLOAT => Some(DXGI_FORMAT_R32_FLOAT),
        _ => None,
    }
}

/// Counts, for the line every ten seconds.
static RAN: [AtomicU32; 4] = [const { AtomicU32::new(0) }; 4];

fn index(role: Role) -> usize {
    match role {
        Role::Resolve => 0,
        Role::Radiance => 1,
        Role::Air => 2,
        Role::Miss => 3,
    }
}

fn say() {
    static LAST: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    let now = std::time::Instant::now();
    let Some(since) = *last else {
        *last = Some(now);
        return;
    };
    let secs = now.duration_since(since).as_secs_f32();
    if secs < 10.0 {
        return;
    }
    *last = Some(now);
    let [resolve, radiance, air, miss] = RAN.each_ref().map(|c| c.swap(0, Ordering::Relaxed));
    let (upscales, cuts) = crate::cutscene::seen();
    tracing::info!(
        "light estimate in the last {secs:.0} s: {resolve} raises, {radiance} keeps, {air} air bounds, {miss} reflections; \
         cutscene look {} (camera cuts so far: {cuts} of {upscales} upscales)",
        if crate::cutscene::now() { "on" } else { "off" },
    );
}

/// After the title's program of `role`, in `list`: the estimate's pass.
/// Sets the list's heaps, compute root signature and pipeline; false if it
/// recorded nothing.
pub(crate) fn after(list: &HostList, role: Role, target: &Target) -> bool {
    let (Some(pass), Some(host), Some(level)) = (pass(), host(), share()) else {
        return false;
    };
    // The share and the constant (for reflections: the share of the title's
    // sky kept).
    let (level, constant) = match role {
        Role::Miss => match specular() {
            Some(s) => (s.share, s.sky),
            None => return false,
        },
        _ => (level, 0.0),
    };
    if role != Role::Radiance && !KEPT.load(Ordering::Relaxed) {
        return false;
    }
    // SAFETY: live resources.
    let image = unsafe { target.image.GetDesc() };
    let depth = target.depth.as_ref().map(|d| (d, unsafe { d.GetDesc() }));
    let volume = image.Dimension == D3D12_RESOURCE_DIMENSION_TEXTURE3D;
    if image.Format != image_format(role) || volume != (role == Role::Air) {
        static TOLD: std::sync::Once = std::sync::Once::new();
        TOLD.call_once(|| tracing::info!(?role, format = image.Format.0, "light estimate: another image; left as the title had it"));
        return false;
    }
    let depth_format = depth.as_ref().and_then(|(_, d)| depth_view(d.Format));
    let needs_depth = matches!(role, Role::Resolve | Role::Miss);
    if needs_depth && depth_format.is_none() {
        return false;
    }
    let depth_size = depth.as_ref().map_or([image.Width as u32, image.Height], |(_, d)| [d.Width as u32, d.Height]);
    static SAID: AtomicU32 = AtomicU32::new(0);
    if SAID.fetch_add(1, Ordering::Relaxed) < 6 {
        tracing::info!(?role, width = image.Width, height = image.Height, level, constant, "light estimate (VIctory-NVIDIA's)");
    }
    list.keep(&pass.cells);
    list.keep(&pass.points);
    list.keep(&target.image);
    if let Some((d, _)) = &depth {
        list.keep(*d);
    }
    // A camera cut of the title's: the slow mean starts again from this
    // frame (the cut is noted at the upscale, late in its frame).
    let frame = crate::frame::current();
    let cut = crate::cutscene::cut_in(frame) || crate::cutscene::cut_in(frame.wrapping_sub(1));
    let [strength, radius, near, tan_y] = super::shade();
    let floats = [level, constant, strength, radius, near, tan_y];
    let mut words = [0u32; 12];
    for (to, v) in words.iter_mut().zip(floats) {
        *to = v.to_bits();
    }
    words[6] = depth_size[0];
    words[7] = depth_size[1];
    words[8] = if cut { 1.0f32 } else { 0.04 }.to_bits();
    words[9] = 4.0f32.to_bits();
    words[10] = air().to_bits();
    words[11] = if once() { 1.0f32 } else { 0.0 }.to_bits();
    let slot = pass.next.fetch_add(1, Ordering::Relaxed) % SLOTS * 2;
    let l = &list.list;
    // SAFETY: an open list; the title's program has just used both
    // textures, so they are in the states these views need (the image as
    // unordered access, the depth as a shader resource); views in a slot no
    // list in flight reads; buffers and textures the list keeps.
    unsafe {
        let cpu = pass.views.GetCPUDescriptorHandleForHeapStart();
        let gpu = pass.views.GetGPUDescriptorHandleForHeapStart();
        let at = |n: u32| D3D12_CPU_DESCRIPTOR_HANDLE { ptr: cpu.ptr + ((slot + n) * pass.step) as usize };
        let seen = D3D12_SHADER_RESOURCE_VIEW_DESC {
            Format: depth_format.unwrap_or(DXGI_FORMAT_R32_FLOAT),
            ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
            Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
            Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                Texture2D: D3D12_TEX2D_SRV { MipLevels: 1, ..Default::default() },
            },
        };
        let depth_resource = depth.as_ref().filter(|_| depth_format.is_some()).map(|(d, _)| *d);
        host.device.CreateShaderResourceView(depth_resource, Some(&seen), at(0));
        let written = if volume {
            D3D12_UNORDERED_ACCESS_VIEW_DESC {
                Format: image.Format,
                ViewDimension: D3D12_UAV_DIMENSION_TEXTURE3D,
                Anonymous: D3D12_UNORDERED_ACCESS_VIEW_DESC_0 {
                    Texture3D: D3D12_TEX3D_UAV { MipSlice: 0, FirstWSlice: 0, WSize: u32::from(image.DepthOrArraySize) },
                },
            }
        } else {
            D3D12_UNORDERED_ACCESS_VIEW_DESC {
                Format: image.Format,
                ViewDimension: D3D12_UAV_DIMENSION_TEXTURE2D,
                ..Default::default()
            }
        };
        host.device.CreateUnorderedAccessView(&target.image, None::<&ID3D12Resource>, Some(&written), at(1));
        let table = |n: u32| D3D12_GPU_DESCRIPTOR_HANDLE { ptr: gpu.ptr + u64::from((slot + n) * pass.step) };
        uav_barrier(l);
        l.SetDescriptorHeaps(&[Some(pass.views.clone())]);
        l.SetComputeRootSignature(&pass.root);
        l.SetComputeRoot32BitConstants(0, 12, words.as_ptr().cast(), 0);
        l.SetComputeRootDescriptorTable(1, table(0));
        l.SetComputeRootDescriptorTable(2, table(1));
        l.SetComputeRootUnorderedAccessView(3, pass.cells.GetGPUVirtualAddress());
        l.SetComputeRootUnorderedAccessView(4, pass.points.GetGPUVirtualAddress());
        let run = |pso: &ID3D12PipelineState, x, y, z| {
            l.SetPipelineState(pso);
            l.Dispatch(x, y, z);
            uav_barrier(l);
        };
        let (w, h) = (image.Width as u32, image.Height);
        match role {
            Role::Radiance => {
                run(&pass.keep, 8, 9, 1);
                run(&pass.mean, 1, 1, 1);
                KEPT.store(true, Ordering::Relaxed);
            }
            Role::Air => run(&pass.air, w.div_ceil(8), h.div_ceil(8), u32::from(image.DepthOrArraySize).div_ceil(2)),
            Role::Resolve => {
                run(&pass.place, 8, 9, 1);
                run(&pass.gather, 20, 45, 1);
                run(&pass.raise, w.div_ceil(8), h.div_ceil(8), 1);
            }
            Role::Miss => {
                // The points afresh, from this frame's depth: the diffuse
                // estimate makes them later in the frame.
                run(&pass.place, 8, 9, 1);
                run(&pass.gather, 20, 45, 1);
                run(&pass.reflect, w.div_ceil(8), h.div_ceil(8), 1);
            }
        }
    }
    RAN[index(role)].fetch_add(1, Ordering::Relaxed);
    say();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings() {
        assert_eq!(parse_share(None), Some(0.8));
        assert_eq!(parse_share(Some("")), Some(0.8));
        assert_eq!(parse_share(Some("0")), None);
        assert_eq!(parse_share(Some("off")), None);
        assert_eq!(parse_share(Some("0.6")), Some(0.6));
        assert_eq!(parse_specular(None), Some(Specular { share: 1.0, sky: 0.0 }));
        assert_eq!(parse_specular(Some("0")), None);
        assert_eq!(parse_specular(Some("off")), None);
        assert_eq!(parse_specular(Some("0.6, 0.25")), Some(Specular { share: 0.6, sky: 0.25 }));
        assert_eq!(parse_specular(Some("0.6,7")), Some(Specular { share: 0.6, sky: 1.0 }));
        assert_eq!(parse_air(None), 1.5);
        assert_eq!(parse_air(Some("0")), 0.0);
        assert_eq!(parse_air(Some("2")), 2.0);
    }

    /// The passes compile with the system's compiler and the host takes them.
    #[test]
    fn the_passes_build() {
        let Some(host) = host() else {
            return;
        };
        make(&host.device, |bytes| host.uav_buffer(bytes)).expect("the passes");
    }
}
