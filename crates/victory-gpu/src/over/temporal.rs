//! The upscaler's output, by a temporal resolve (Try 6.7).
//!
//! The plain stretch (`super::record_stretch`) shows one frame's samples
//! and nothing more. The title draws hair with dithered transparency and
//! its soft shadows and screen-space passes with a few jittered samples a
//! texel, counting on the console's upscaler to average them over frames.
//! Shown one frame at a time they are dotted, see-through and shimmering.
//!
//! This pass does what that upscaler's accumulation does, in one compute
//! dispatch (the TAA/TAAU of the 2010s engines):
//!
//! * the colour input is sampled where the texel's sample really was:
//!   the jitter the title passes is taken off;
//! * the image built so far (the history, at output size, ours) is moved
//!   along the title's motion vectors to where this frame sees it, with a
//!   Catmull-Rom filter, so it stays sharp;
//! * the history is clipped to the colours of this frame's 3x3
//!   neighbourhood (variance clipping in YCoCg, within its minimum and
//!   maximum), so what moved or was uncovered does not trail;
//! * the two are blended, a tenth of the new frame by default, in a
//!   tonemapped space so lone bright texels do not flash;
//! * a little of the detail against the neighbourhood's mean is added
//!   back (sharpening, the title's own amount when it asks for it).
//!
//! The history is two buffers of half-precision texels, ping-ponged:
//! buffers promote to unordered access by themselves and decay back after
//! each submission, so no state of ours is tracked. It starts again when
//! the output size changes, when the title asks (a camera cut), and after
//! a long gap between dispatches (a loading screen).
//!
//! `VICTORY_UPSCALE_TEMPORAL=0` brings back the plain stretch;
//! `VICTORY_UPSCALE_FEEDBACK` (percent of the new frame, 10) and
//! `VICTORY_UPSCALE_SHARPEN` (percent, 20) tune it.

use super::*;

/// The resolve, compiled with the system compiler (`crate::host::compile`).
pub(super) const RESOLVE: &str = r#"
cbuffer Sizes : register(b0) {
    float2 src_size; float2 rendered;
    float2 out_size; float2 jitter;
    float2 mv_scale; float2 mv_size;
    float feedback; float sharpen; float exposure; uint flags;
};
Texture2D<float4> source : register(t0);
Texture2D<float4> motion : register(t1);
RWTexture2D<float4> target : register(u0);
RWStructuredBuffer<uint2> before : register(u1);
RWStructuredBuffer<uint2> after : register(u2);
SamplerState bilinear : register(s0);

float3 clean(float3 c) { return (c == c && abs(c) < 65000.0) ? max(c, 0.0) : 0.0; }
float most(float3 c) { return max(c.r, max(c.g, c.b)); }
float3 squash(float3 c) { return c / (1.0 + most(c)); }
float3 unsquash(float3 c) { return c / max(1.0 - most(c), 0.001); }
float3 to_ycocg(float3 c) {
    return float3(dot(c, float3(0.25, 0.5, 0.25)), dot(c, float3(0.5, 0.0, -0.5)), dot(c, float3(-0.25, 0.5, -0.25)));
}
float3 from_ycocg(float3 c) { return float3(c.x + c.y - c.z, c.x + c.z, c.x - c.y - c.z); }
float4 unpack(uint2 v) {
    return float4(f16tof32(v.x), f16tof32(v.x >> 16), f16tof32(v.y), f16tof32(v.y >> 16));
}
uint2 pack(float4 c) {
    return uint2(f32tof16(c.x) | (f32tof16(c.y) << 16), f32tof16(c.z) | (f32tof16(c.w) << 16));
}
float3 history_at(int2 p) {
    p = clamp(p, int2(0, 0), int2(out_size) - 1);
    return clean(unpack(before[uint(p.y) * uint(out_size.x) + uint(p.x)]).rgb);
}
// Catmull-Rom over 4x4 history texels around `pos` (output pixels).
float3 catmull(float2 pos) {
    float2 f = pos - 0.5;
    float2 i = floor(f);
    float2 t = f - i;
    float2 w[4];
    w[0] = t * (-0.5 + t * (1.0 - 0.5 * t));
    w[1] = 1.0 + t * t * (-2.5 + 1.5 * t);
    w[2] = t * (0.5 + t * (2.0 - 1.5 * t));
    w[3] = t * t * (-0.5 + 0.5 * t);
    float3 sum = 0.0;
    [unroll] for (int y = 0; y < 4; y++) {
        [unroll] for (int x = 0; x < 4; x++) {
            sum += history_at(int2(i) + int2(x - 1, y - 1)) * (w[x].x * w[y].y);
        }
    }
    return max(sum, 0.0);
}
[numthreads(8, 8, 1)]
void resolve(uint3 id : SV_DispatchThreadID) {
    // Groups are 8 by 8: the last ones reach past a size that is not a
    // multiple of 8.
    uint width, height;
    target.GetDimensions(width, height);
    if (id.x >= width || id.y >= height) return;
    float2 out_pos = float2(id.xy) + 0.5;
    // Where this output texel is in the rendered image, without jitter;
    // the rendered texel whose sample is nearest it.
    float2 src_pos = out_pos * rendered / out_size;
    int2 last = int2(rendered) - 1;
    int2 centre = clamp(int2(floor(src_pos + jitter)), int2(0, 0), last);
    // The neighbourhood: the clip box and the mean.
    float3 m1 = 0.0, m2 = 0.0, lo = 1e30, hi = -1e30;
    [unroll] for (int y = -1; y <= 1; y++) {
        [unroll] for (int x = -1; x <= 1; x++) {
            int2 p = clamp(centre + int2(x, y), int2(0, 0), last);
            float3 c = to_ycocg(squash(clean(source.Load(int3(p, 0)).rgb)));
            m1 += c;
            m2 += c * c;
            lo = min(lo, c);
            hi = max(hi, c);
        }
    }
    float3 mean = m1 / 9.0;
    float3 sigma = sqrt(max(m2 / 9.0 - mean * mean, 0.0));
    float3 box_lo = max(lo, mean - 1.25 * sigma);
    float3 box_hi = min(hi, mean + 1.25 * sigma);
    // This frame's colour, filtered at the unjittered place.
    float2 at = (src_pos + jitter) / src_size;
    at = clamp(at, 0.5 / src_size, (rendered - 0.5) / src_size);
    float4 now = source.SampleLevel(bilinear, at, 0);
    float3 current = to_ycocg(squash(clean(now.rgb)));
    // Motion: the longest of a cross around the texel, so the edges of
    // what moves carry its motion (UV, current to previous).
    float2 mv = 0.0;
    if (flags & 2) {
        int2 mlast = int2(mv_size) - 1;
        float best = -1.0;
        int2 taps[5] = { int2(0, 0), int2(1, 0), int2(-1, 0), int2(0, 1), int2(0, -1) };
        [unroll] for (int k = 0; k < 5; k++) {
            int2 p = clamp(centre + taps[k], int2(0, 0), mlast);
            float2 m = motion.Load(int3(p, 0)).xy * mv_scale;
            m = (m == m && abs(m) < 4.0) ? m : 0.0;
            float l = dot(m, m);
            if (l > best) { best = l; mv = m; }
        }
    }
    float2 prev = out_pos / out_size + mv;
    bool valid = (flags & 1) == 0 && all(prev >= 0.0) && all(prev <= 1.0);
    float alpha = feedback;
    float3 result = current;
    if (valid) {
        float3 h = to_ycocg(squash(catmull(prev * out_size) * exposure));
        h = clamp(h, box_lo, box_hi);
        // Fast motion keeps less of the history: the filter softens it.
        alpha = max(alpha, saturate(length(mv * out_size) * 0.01));
        result = lerp(h, current, alpha);
    }
    float3 rgb = from_ycocg(result);
    rgb = max(rgb + sharpen * (rgb - from_ycocg(mean)), 0.0);
    rgb = min(rgb, 0.995);
    float3 linear_rgb = unsquash(rgb);
    after[id.y * width + id.x] = pack(float4(min(linear_rgb, 65000.0), 1.0));
    target[id.xy] = float4(linear_rgb, now.a);
}
"#;

pub(super) struct Temporal {
    root: ID3D12RootSignature,
    pipeline: ID3D12PipelineState,
    views: ID3D12DescriptorHeap,
    step: u32,
    next: u32,
    /// The two history buffers and the output size they are for.
    history: Option<([ID3D12Resource; 2], (u32, u32))>,
    /// Which history buffer the next resolve reads.
    read: usize,
    /// When the last resolve was recorded, for the gap that starts the
    /// history again.
    last: Option<std::time::Instant>,
    /// The title's pre-exposure at the last resolve.
    exposure: f32,
}

// SAFETY: D3D12 objects are free-threaded.
unsafe impl Send for Temporal {}

pub(super) static TEMPORAL: Mutex<Option<Temporal>> = Mutex::new(None);

/// Views of a resolve are kept this many resolves (five views each), as
/// the stretch's are (`STRETCH_RING`).
const RING: u32 = 64;
const VIEWS: u32 = 5;

/// A gap between two resolves longer than this starts the history again:
/// what the last one saw is from before a loading screen.
const GAP_SECS: u64 = 5;

/// Whether the upscaler's output is made by the temporal resolve
/// (`VICTORY_UPSCALE_TEMPORAL`, on unless `0`).
pub(crate) fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("VICTORY_UPSCALE_TEMPORAL").is_none_or(|v| v != "0"))
}

fn percent(name: &str, default: f32, low: f32, high: f32) -> f32 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<f32>().ok())
        .map_or(default, |p| (p / 100.0).clamp(low, high))
}

/// How much of the new frame each resolve keeps (`VICTORY_UPSCALE_FEEDBACK`).
fn feedback() -> f32 {
    static F: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *F.get_or_init(|| percent("VICTORY_UPSCALE_FEEDBACK", 0.1, 0.02, 1.0))
}

/// The sharpening amount, unless the title asks for its own
/// (`VICTORY_UPSCALE_SHARPEN`).
fn sharpen(title: Option<f32>) -> f32 {
    static S: std::sync::OnceLock<Option<f32>> = std::sync::OnceLock::new();
    let set = *S.get_or_init(|| {
        std::env::var("VICTORY_UPSCALE_SHARPEN")
            .ok()
            .and_then(|v| v.trim().parse::<f32>().ok())
            .map(|p| (p / 100.0).clamp(0.0, 1.0))
    });
    set.or(title.map(|s| (s * 0.5).clamp(0.0, 0.5))).unwrap_or(0.2)
}

/// Colour formats whose two first channels a float view reads as motion.
fn motion_format(format: DXGI_FORMAT) -> bool {
    // R32G32B32A32_FLOAT, R16G16B16A16_FLOAT, R32G32_FLOAT, R16G16_FLOAT,
    // R16G16_SNORM.
    matches!(format.0, 2 | 10 | 16 | 34 | 37)
}

fn ranges() -> [D3D12_DESCRIPTOR_RANGE; 2] {
    [
        D3D12_DESCRIPTOR_RANGE {
            RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
            NumDescriptors: 2,
            BaseShaderRegister: 0,
            RegisterSpace: 0,
            OffsetInDescriptorsFromTableStart: 0,
        },
        D3D12_DESCRIPTOR_RANGE {
            RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_UAV,
            NumDescriptors: 3,
            BaseShaderRegister: 0,
            RegisterSpace: 0,
            OffsetInDescriptorsFromTableStart: 2,
        },
    ]
}

fn make(device: &ID3D12Device) -> windows::core::Result<Temporal> {
    use windows::core::s;
    let cs = crate::host::compile(RESOLVE, s!("resolve"), s!("cs_5_1"))?;
    let views = ranges();
    let sizes = D3D12_ROOT_PARAMETER {
        ParameterType: D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS,
        Anonymous: D3D12_ROOT_PARAMETER_0 {
            Constants: D3D12_ROOT_CONSTANTS {
                ShaderRegister: 0,
                RegisterSpace: 0,
                Num32BitValues: 16,
            },
        },
        ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
    };
    let root = root_signature(
        device,
        &[sizes, table(&views, D3D12_SHADER_VISIBILITY_ALL)],
        D3D12_SHADER_VISIBILITY_ALL,
        D3D12_ROOT_SIGNATURE_FLAG_NONE,
    )?;
    // SAFETY: the description points at live data and never releases its
    // pointer.
    unsafe {
        let desc = D3D12_COMPUTE_PIPELINE_STATE_DESC {
            pRootSignature: std::mem::transmute_copy(&root),
            CS: D3D12_SHADER_BYTECODE {
                pShaderBytecode: cs.as_ptr().cast(),
                BytecodeLength: cs.len(),
            },
            ..Default::default()
        };
        let pipeline: ID3D12PipelineState = device.CreateComputePipelineState(&desc)?;
        let views: ID3D12DescriptorHeap =
            device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                NumDescriptors: RING * VIEWS,
                Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
                NodeMask: 0,
            })?;
        Ok(Temporal {
            root,
            pipeline,
            step: device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV),
            views,
            next: 0,
            history: None,
            read: 0,
            last: None,
            exposure: 1.0,
        })
    }
}

/// A history buffer of `texels` half-precision RGBA texels.
fn history_buffer(device: &ID3D12Device, texels: u64) -> windows::core::Result<ID3D12Resource> {
    let heap = D3D12_HEAP_PROPERTIES {
        Type: D3D12_HEAP_TYPE_DEFAULT,
        ..Default::default()
    };
    let desc = D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
        Alignment: 0,
        Width: texels * 8,
        Height: 1,
        DepthOrArraySize: 1,
        MipLevels: 1,
        Format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_UNKNOWN,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
        Flags: D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS,
    };
    let mut made: Option<ID3D12Resource> = None;
    // SAFETY: live descriptions; the out pointer is ours.
    unsafe {
        device.CreateCommittedResource(
            &heap,
            D3D12_HEAP_FLAG_NONE,
            &desc,
            D3D12_RESOURCE_STATE_COMMON,
            None,
            &mut made,
        )?;
    }
    made.ok_or_else(windows::core::Error::empty)
}

/// What one upscale dispatch gives the resolve besides its textures.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame {
    pub render: [u32; 2],
    pub jitter: [f32; 2],
    pub motion_scale: [f32; 2],
    pub reset: bool,
    pub sharpness: Option<f32>,
    pub pre_exposure: f32,
}

/// Records into `list` the temporal resolve of the top-left `frame.render`
/// texels of `source` over the whole of `target` (in the unordered-access
/// state), moved by `motion` when there is one. False if nothing was
/// recorded; the caller then stretches.
///
/// The pass sets the list's descriptor heap, compute root signature and
/// pipeline and puts none of them back: the caller replays the title's.
pub(crate) fn record(
    device: &ID3D12Device,
    host: &crate::host::HostList,
    target: &ID3D12Resource,
    source: &ID3D12Resource,
    motion: Option<&ID3D12Resource>,
    frame: Frame,
) -> bool {
    let list = &host.list;
    // SAFETY: live resources.
    let (to, from) = unsafe { (target.GetDesc(), source.GetDesc()) };
    if to.Flags.0 & D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS.0 == 0
        || to.Width == 0
        || to.Height == 0
        || from.Width == 0
        || from.Height == 0
    {
        return false;
    }
    let mut made = TEMPORAL.lock().unwrap_or_else(|e| e.into_inner());
    if made.is_none() {
        *made = match make(device) {
            Ok(p) => {
                tracing::info!(
                    feedback = feedback(),
                    "the upscaler's output is made by a temporal resolve: jitter taken off, history moved by the title's motion, clipped to the neighbourhood"
                );
                Some(p)
            }
            Err(e) => {
                static TOLD: std::sync::Once = std::sync::Once::new();
                TOLD.call_once(|| {
                    tracing::warn!(error = %e, "no temporal resolve: the upscaler's output is stretched");
                });
                None
            }
        };
    }
    let Some(p) = made.as_mut() else { return false };
    let (width, height) = (to.Width as u32, to.Height);
    let mut reset = frame.reset;
    if p.history.as_ref().map(|h| h.1) != Some((width, height)) {
        let texels = u64::from(width) * u64::from(height);
        match (history_buffer(device, texels), history_buffer(device, texels)) {
            (Ok(a), Ok(b)) => {
                // The old buffers can still be read by lists in flight.
                if let Some((old, _)) = p.history.take() {
                    host.keep(&old[0]);
                    host.keep(&old[1]);
                }
                tracing::info!(width, height, "temporal resolve history made");
                p.history = Some(([a, b], (width, height)));
                reset = true;
            }
            (Err(e), _) | (_, Err(e)) => {
                static TOLD: std::sync::Once = std::sync::Once::new();
                TOLD.call_once(|| {
                    tracing::warn!(error = %e, "no memory for the temporal resolve's history: the upscaler's output is stretched");
                });
                return false;
            }
        }
    }
    let now = std::time::Instant::now();
    if p.last.is_none_or(|t| now.duration_since(t).as_secs() >= GAP_SECS) {
        reset = true;
    }
    p.last = Some(now);
    let exposure_now = if frame.pre_exposure.is_finite() && frame.pre_exposure > 0.0 {
        frame.pre_exposure
    } else {
        1.0
    };
    let exposure = if reset { 1.0 } else { (exposure_now / p.exposure).clamp(0.01, 100.0) };
    p.exposure = exposure_now;
    let Some((history, _)) = p.history.as_ref() else { return false };
    let (before, after) = (&history[p.read], &history[1 - p.read]);
    p.read = 1 - p.read;
    let slot = (p.next % RING) * VIEWS;
    p.next = p.next.wrapping_add(1);
    let rendered = [
        if frame.render[0] == 0 { from.Width as u32 } else { frame.render[0].min(from.Width as u32) },
        if frame.render[1] == 0 { from.Height } else { frame.render[1].min(from.Height) },
    ];
    // Motion: a float view of its two first channels, scaled to UV.
    // SAFETY: a live resource.
    let motion = motion.map(|m| (m, unsafe { m.GetDesc() })).filter(|(_, d)| {
        let ok = motion_format(typed(d.Format)) && d.Width > 0 && d.Height > 0;
        if !ok {
            static TOLD: std::sync::Once = std::sync::Once::new();
            TOLD.call_once(|| {
                tracing::info!(format = d.Format.0, "the upscaler's motion input is not read: the resolve keeps still history");
            });
        }
        ok
    });
    let (mv_scale, mv_size) = match &motion {
        Some((_, d)) => (
            [
                frame.motion_scale[0] / rendered[0] as f32,
                frame.motion_scale[1] / rendered[1] as f32,
            ],
            [(d.Width as u32).min(rendered[0]) as f32, d.Height.min(rendered[1]) as f32],
        ),
        None => ([0.0, 0.0], [1.0, 1.0]),
    };
    let jitter = if frame.jitter.iter().all(|j| j.is_finite() && j.abs() <= 2.0) {
        frame.jitter
    } else {
        [0.0, 0.0]
    };
    let flags = u32::from(reset) | (u32::from(motion.is_some()) << 1);
    let constants: [u32; 16] = [
        (from.Width as f32).to_bits(),
        (from.Height as f32).to_bits(),
        (rendered[0] as f32).to_bits(),
        (rendered[1] as f32).to_bits(),
        (width as f32).to_bits(),
        (height as f32).to_bits(),
        jitter[0].to_bits(),
        jitter[1].to_bits(),
        mv_scale[0].to_bits(),
        mv_scale[1].to_bits(),
        mv_size[0].to_bits(),
        mv_size[1].to_bits(),
        feedback().to_bits(),
        sharpen(frame.sharpness).to_bits(),
        exposure.to_bits(),
        flags,
    ];
    let read = |format: DXGI_FORMAT| D3D12_SHADER_RESOURCE_VIEW_DESC {
        Format: typed(format),
        ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
        Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
        Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
            Texture2D: D3D12_TEX2D_SRV {
                MostDetailedMip: 0,
                MipLevels: 1,
                ..Default::default()
            },
        },
    };
    let written = D3D12_UNORDERED_ACCESS_VIEW_DESC {
        Format: typed(to.Format),
        ViewDimension: D3D12_UAV_DIMENSION_TEXTURE2D,
        ..Default::default()
    };
    let texels = u32::try_from(u64::from(width) * u64::from(height)).unwrap_or(u32::MAX);
    let buffer_view = D3D12_UNORDERED_ACCESS_VIEW_DESC {
        Format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_UNKNOWN,
        ViewDimension: D3D12_UAV_DIMENSION_BUFFER,
        Anonymous: D3D12_UNORDERED_ACCESS_VIEW_DESC_0 {
            Buffer: D3D12_BUFFER_UAV {
                FirstElement: 0,
                NumElements: texels,
                StructureByteStride: 8,
                CounterOffsetInBytes: 0,
                Flags: D3D12_BUFFER_UAV_FLAG_NONE,
            },
        },
    };
    // SAFETY: an open list; slots of our heap; live resources and objects
    // (the pass's own live as long as the process, or are kept by the
    // list when replaced).
    unsafe {
        let cpu = |i: u32| D3D12_CPU_DESCRIPTOR_HANDLE {
            ptr: p.views.GetCPUDescriptorHandleForHeapStart().ptr + ((slot + i) * p.step) as usize,
        };
        let gpu = D3D12_GPU_DESCRIPTOR_HANDLE {
            ptr: p.views.GetGPUDescriptorHandleForHeapStart().ptr + u64::from(slot * p.step),
        };
        device.CreateShaderResourceView(source, Some(&read(from.Format)), cpu(0));
        match &motion {
            Some((m, d)) => device.CreateShaderResourceView(*m, Some(&read(d.Format)), cpu(1)),
            // Never read (the flag is off), but every view of the table is
            // a valid one.
            None => device.CreateShaderResourceView(source, Some(&read(from.Format)), cpu(1)),
        }
        device.CreateUnorderedAccessView(target, None, Some(&written), cpu(2));
        device.CreateUnorderedAccessView(before, None, Some(&buffer_view), cpu(3));
        device.CreateUnorderedAccessView(after, None, Some(&buffer_view), cpu(4));
        // The last resolve's writes of the history are done before this
        // one reads them (a list can hold two resolves).
        list.ResourceBarrier(&[uav_barrier(before)]);
        list.SetDescriptorHeaps(&[Some(p.views.clone())]);
        list.SetComputeRootSignature(&p.root);
        list.SetPipelineState(&p.pipeline);
        list.SetComputeRoot32BitConstants(0, 16, constants.as_ptr().cast(), 0);
        list.SetComputeRootDescriptorTable(1, gpu);
        list.Dispatch(width.div_ceil(8), height.div_ceil(8), 1);
    }
    // How the history fares, now and then: resolves that started it
    // again are frames shown without accumulation.
    use std::sync::atomic::Ordering::Relaxed;
    let resets = RESETS.load(Relaxed) + u64::from(reset);
    RESETS.store(resets, Relaxed);
    let resolves = RESOLVES.fetch_add(1, Relaxed) + 1;
    if resolves.is_power_of_two() || resolves.is_multiple_of(256) {
        tracing::info!(
            resolves,
            resets,
            motion = motion.is_some(),
            jitter = ?jitter,
            mv_scale = ?mv_scale,
            exposure,
            "temporal resolves"
        );
    }
    true
}

static RESOLVES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static RESETS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn uav_barrier(resource: &ID3D12Resource) -> D3D12_RESOURCE_BARRIER {
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_UAV,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            UAV: std::mem::ManuallyDrop::new(D3D12_RESOURCE_UAV_BARRIER {
                // SAFETY: a borrowed pointer; the barrier never releases it.
                pResource: unsafe { std::mem::transmute_copy(resource) },
            }),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The resolve's source asks the compiler for no root signature and
    /// guards its writes, as the stretch's does.
    #[test]
    fn the_resolve_is_a_guarded_compute_pass_without_a_root_signature() {
        assert!(!RESOLVE.contains("RootSignature"));
        assert!(RESOLVE.contains("numthreads(8, 8, 1)"));
        assert!(RESOLVE.contains("if (id.x >= width || id.y >= height) return;"));
    }

    /// The table holds the five views the source declares: two textures
    /// read, the output and the two history buffers written.
    #[test]
    fn the_table_matches_the_source() {
        let r = ranges();
        assert_eq!(r[0].NumDescriptors + r[1].NumDescriptors, VIEWS);
        for register in ["t0", "t1", "u0", "u1", "u2"] {
            assert!(RESOLVE.contains(&format!("register({register})")));
        }
    }

    /// Only float views of motion are read.
    #[test]
    fn motion_is_read_from_float_formats_only() {
        for f in [2, 10, 16, 34, 37] {
            assert!(motion_format(DXGI_FORMAT(f)));
        }
        // R32_UINT, R16G16_UINT, R8G8B8A8_UNORM.
        for f in [42, 36, 28] {
            assert!(!motion_format(DXGI_FORMAT(f)));
        }
    }
}
