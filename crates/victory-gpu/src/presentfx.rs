//! What the present does to the title's frames on their way to the screen
//! (Try 4.2), a child of `host` so it shares the swap chain's helpers:
//!
//! * **Render scale** (idea 6, after the DLSS/FSR enabler mods):
//!   `VICTORY_RENDER_SCALE` below 100 tells the title a smaller display
//!   (`crate::dxgi::render_scale`), so it renders every pass at that
//!   size, and here each frame is scaled up to the screen's size with a
//!   Catmull-Rom filter, clamped to the four nearest texels as FSR's EASU
//!   is (no rings), and sharpened with FSR's RCAS (contrast-adaptive:
//!   edges and texture detail get their contrast back, flat areas and
//!   noise are left alone; `VICTORY_SHARPEN`, percent, 80 by default since
//!   Try 5.8).
//! * **Frames in between** (idea 7, after VR's asynchronous spacewarp):
//!   with `VICTORY_FRAMEGEN=1`, each new frame of the title is compared
//!   with the one before, block by block (16x16 pixels, a search of ±20
//!   pixels coarse and then to the pixel), and while the title works on
//!   its next frame, a frame is shown every `VICTORY_FRAMEGEN_MS` (33 by
//!   default) that moves the last one on along that motion, as far as the
//!   time since it was shown is of the time between the title's frames.
//!   The camera and what moves keep moving on the screen between the
//!   title's frames. It adds no frames of the title's own: the frame-rate
//!   line still counts only those.
//!
//! How: the title's frame is copied on the host's queue into a ring of
//! three textures of ours, and a fence is signalled behind the copy. The
//! swap chain is on a queue of its own: it waits for that fence, runs our
//! passes and presents, so the frames made in between never wait behind
//! the title's work. Before a ring texture is written again the host's
//! queue waits until our queue is done with it.
//!
//! Anything that fails while setting up leaves the present as it was.

use super::*;

use std::sync::atomic::{AtomicBool, AtomicU64};

/// The passes, compiled with the system's compiler (`super::compile`).
const SOURCE: &str = r#"
#define RS "RootConstants(num32BitConstants=12, b0), DescriptorTable(SRV(t0, numDescriptors=2), UAV(u0, numDescriptors=2))"
cbuffer C : register(b0) {
    uint2 inSize;
    uint2 outSize;
    float amount;
    float sharp;
    uint useMotion;
    uint blocksX;
    uint blocksY;
    uint3 pad;
};
Texture2D<float4> cur : register(t0);
Texture2D<float4> prev : register(t1);
RWTexture2D<float4> outTex : register(u0);
RWStructuredBuffer<float2> motion : register(u1);

#define BLOCK 16

float luma(float3 c) { return dot(c, float3(0.299, 0.587, 0.114)); }

float3 texelOf(Texture2D<float4> t, int2 p) {
    p = clamp(p, int2(0, 0), int2(inSize) - 1);
    return t.Load(int3(p, 0)).rgb;
}

groupshared uint best;

uint cost(int2 origin, int2 d) {
    float s = 0;
    [loop] for (int y = 0; y < BLOCK; y += 2) {
        [loop] for (int x = 0; x < BLOCK; x += 2) {
            int2 p = origin + int2(x, y);
            s += abs(luma(texelOf(cur, p)) - luma(texelOf(prev, p + d)));
        }
    }
    // A small toll per pixel of motion: flat areas stay still.
    s += 0.004 * (abs(d.x) + abs(d.y));
    return min((uint)(s * 4096.0), 0x1ffffffu);
}

[RootSignature(RS)]
[numthreads(9, 9, 1)]
void motionMain(uint3 g : SV_GroupID, uint3 t : SV_GroupThreadID, uint i : SV_GroupIndex) {
    int2 origin = int2(g.xy) * BLOCK;
    if (i == 0) best = 0xffffffffu;
    GroupMemoryBarrierWithGroupSync();
    int2 d = (int2(t.xy) - 4) * 5;
    InterlockedMin(best, (cost(origin, d) << 7) | i);
    GroupMemoryBarrierWithGroupSync();
    uint k = best & 127;
    int2 coarse = (int2(k % 9, k / 9) - 4) * 5;
    GroupMemoryBarrierWithGroupSync();
    if (i == 0) best = 0xffffffffu;
    GroupMemoryBarrierWithGroupSync();
    d = coarse + int2(t.xy) - 4;
    InterlockedMin(best, (cost(origin, d) << 7) | i);
    GroupMemoryBarrierWithGroupSync();
    if (i == 0) {
        uint k2 = best & 127;
        int2 fine = coarse + int2(k2 % 9, k2 / 9) - 4;
        motion[g.y * blocksX + g.x] = float2(fine);
    }
}

float2 motionAt(float2 p) {
    float2 b = p / BLOCK - 0.5;
    int2 b0 = (int2)floor(b);
    float2 f = b - float2(b0);
    int2 most = int2(blocksX, blocksY) - 1;
    int2 a = clamp(b0, int2(0, 0), most);
    int2 c = clamp(b0 + 1, int2(0, 0), most);
    float2 m00 = motion[a.y * blocksX + a.x];
    float2 m10 = motion[a.y * blocksX + c.x];
    float2 m01 = motion[c.y * blocksX + a.x];
    float2 m11 = motion[c.y * blocksX + c.x];
    return lerp(lerp(m00, m10, f.x), lerp(m01, m11, f.x), f.y);
}

// The title's frame at output pixel `o` (pixel centres at +0.5): a
// Catmull-Rom filter clamped to the four nearest texels, as FSR's EASU is
// (no rings).
float3 upscaled(int2 o) {
    o = clamp(o, int2(0, 0), int2(outSize) - 1);
    float2 p = (float2(o) + 0.5) * float2(inSize) / float2(outSize);
    if (useMotion != 0) p += motionAt(p) * amount;
    float2 q = p - 0.5;
    int2 i = (int2)floor(q);
    float2 f = q - float2(i);
    float2 w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
    float2 w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
    float2 w2 = f * (0.5 + f * (2.0 - 1.5 * f));
    float2 w3 = f * f * (-0.5 + 0.5 * f);
    float wx[4] = { w0.x, w1.x, w2.x, w3.x };
    float wy[4] = { w0.y, w1.y, w2.y, w3.y };
    float3 c = 0;
    [unroll] for (int y = 0; y < 4; y++) {
        [unroll] for (int x = 0; x < 4; x++) {
            c += texelOf(cur, i + int2(x - 1, y - 1)) * (wx[x] * wy[y]);
        }
    }
    float3 s00 = texelOf(cur, i);
    float3 s10 = texelOf(cur, i + int2(1, 0));
    float3 s01 = texelOf(cur, i + int2(0, 1));
    float3 s11 = texelOf(cur, i + int2(1, 1));
    float3 lo = min(min(s00, s10), min(s01, s11));
    float3 hi = max(max(s00, s10), max(s01, s11));
    return saturate(clamp(c, lo, hi));
}

// Try 5.8: then sharpened as FSR 1's RCAS does (robust contrast-adaptive
// sharpening, AMD, MIT licence): each pixel against its four neighbours,
// as hard as it can be without going past their range (no halos), less
// where the contrast is high already. The old pass sharpened against a
// bilinear blur and clamped the result back to the four nearest texels,
// which took most of it away again.
[RootSignature(RS)]
[numthreads(8, 8, 1)]
void resolveMain(uint3 id : SV_DispatchThreadID) {
    if (id.x >= outSize.x || id.y >= outSize.y) return;
    int2 o = int2(id.xy);
    float3 e = upscaled(o);
    if (sharp > 0) {
        float3 b = upscaled(o + int2(0, -1));
        float3 d = upscaled(o + int2(-1, 0));
        float3 fr = upscaled(o + int2(1, 0));
        float3 h = upscaled(o + int2(0, 1));
        float3 mn4 = min(min(b, d), min(fr, h));
        float3 mx4 = max(max(b, d), max(fr, h));
        float3 hitMin = mn4 / (4.0 * mx4 + 1e-5);
        float3 hitMax = (1.0 - mx4) / (4.0 * mn4 - 4.0 - 1e-5);
        float3 lobeRGB = max(-hitMin, hitMax);
        float lobe = max(-0.1875, min(max(lobeRGB.r, max(lobeRGB.g, lobeRGB.b)), 0.0)) * sharp;
        e = (lobe * (b + d + fr + h) + e) / (4.0 * lobe + 1.0);
    }
    outTex[id.xy] = float4(saturate(e), 1.0);
}
"#;

/// Whether frames are made between the title's (`VICTORY_FRAMEGEN=1`).
/// Not when DLSS Frame Generation runs (Try 6.24: the same switch asks for
/// it, `crate::streamline`), which makes its frames in Streamline's present;
/// ours then stays out of it. Asked first at the first present, after the
/// swap chain was made, so whether NVIDIA's is running is known.
fn framegen() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        let asked = std::env::var_os("VICTORY_FRAMEGEN").is_some_and(|v| v == "1");
        // VICTORY_FRAMEGEN_X names DLSS Frame Generation's amount (Try 6.24
        // sets it whenever it makes DLSS's ready): ours stays off then, even
        // if Streamline does not come up.
        if asked && std::env::var_os("VICTORY_FRAMEGEN_X").is_some() {
            tracing::info!("frames in between: left to DLSS Frame Generation (VICTORY_FRAMEGEN_X)");
            return false;
        }
        #[cfg(windows)]
        if asked && crate::streamline::framegen_live() {
            tracing::info!("frames in between: DLSS Frame Generation makes them (ours stays off)");
            return false;
        }
        asked
    })
}

/// Milliseconds between frames made in between (`VICTORY_FRAMEGEN_MS`).
fn framegen_ms() -> u64 {
    static MS: OnceLock<u64> = OnceLock::new();
    *MS.get_or_init(|| {
        std::env::var("VICTORY_FRAMEGEN_MS")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map_or(33, |ms| ms.clamp(8, 200))
    })
}

/// How much the scaled frame is sharpened, 0 to 1 (`VICTORY_SHARPEN`,
/// percent; 80 by default since 5.8, RCAS's own default strength).
fn sharpen() -> f32 {
    static SHARP: OnceLock<f32> = OnceLock::new();
    *SHARP.get_or_init(|| {
        std::env::var("VICTORY_SHARPEN")
            .ok()
            .and_then(|v| v.trim().parse::<u32>().ok())
            .map_or(80, |p| p.min(100)) as f32
            / 100.0
    })
}

/// Whether the present goes through here.
pub(super) fn on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| framegen() || crate::dxgi::output_size().is_some()) && !FAILED.load(Ordering::Relaxed)
}

/// Setting up failed: the present is as it was for the rest of the run.
static FAILED: AtomicBool = AtomicBool::new(false);

/// Frames made in between since the last line about them.
static MADE: AtomicU64 = AtomicU64::new(0);

/// Blocks of the motion field at most (a 4096x2304 frame).
const BLOCKS: u32 = 256 * 144;

/// Ring textures.
const RING: usize = 3;

/// Descriptor sets for the ring's generations (each set: the frame, the
/// one before, the output, the motion field; one set per ring texture).
const GENERATIONS: u32 = 16;

/// Our queue's lists.
const LISTS: usize = 8;

struct Fx {
    queue: ID3D12CommandQueue,
    swap_chain: IDXGISwapChain3,
    hwnd: HWND,
    out_size: (u32, u32),
    root: ID3D12RootSignature,
    motion_pso: ID3D12PipelineState,
    resolve_pso: ID3D12PipelineState,
    views: ID3D12DescriptorHeap,
    view_cpu: usize,
    view_gpu: u64,
    step: u32,
    out: ID3D12Resource,
    motion: ID3D12Resource,
    /// The ring of the title's frames, their size and format, and the
    /// descriptor generation they are in.
    ring: Vec<ID3D12Resource>,
    in_size: (u32, u32),
    in_format: DXGI_FORMAT,
    generation: u32,
    /// Ring textures of earlier sizes: lists of ours may still read them.
    retired: Vec<ID3D12Resource>,
    /// The value of `done` after our queue's last use of each ring texture.
    used: [u64; RING],
    /// The host queue's lists for the copies, with the submission each
    /// last flew as, and the one for copies waited for at once.
    copies: Vec<(HostList, u64)>,
    copy_now: Option<HostList>,
    /// Signalled on the host's queue behind each copy.
    ready: ID3D12Fence,
    ready_value: u64,
    /// Signalled on our queue behind each list.
    done: ID3D12Fence,
    done_value: u64,
    lists: Vec<(ID3D12CommandAllocator, ID3D12GraphicsCommandList, u64)>,
    event: HANDLE,
    /// The title's frames copied so far.
    frames: u64,
    /// `done`'s value behind the last of them, when it was seen shown,
    /// and the time between the title's frames (seconds, smoothed).
    last_real: u64,
    shown: Option<std::time::Instant>,
    last_present: Option<std::time::Instant>,
    interval: f64,
    logged: std::time::Instant,
    meter: Meter,
}

// SAFETY: D3D12 and DXGI objects are free-threaded; the window handle is
// only passed to the system; everything is used under `FX`'s lock.
unsafe impl Send for Fx {}

static FX: Mutex<Option<Fx>> = Mutex::new(None);

fn texture(device: &ID3D12Device, (width, height): (u32, u32), format: DXGI_FORMAT, flags: D3D12_RESOURCE_FLAGS) -> windows::core::Result<ID3D12Resource> {
    let heap = D3D12_HEAP_PROPERTIES {
        Type: D3D12_HEAP_TYPE_DEFAULT,
        ..Default::default()
    };
    let desc = D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
        Alignment: 0,
        Width: u64::from(width),
        Height: height,
        DepthOrArraySize: 1,
        MipLevels: 1,
        Format: format,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
        Flags: flags,
    };
    let mut made: Option<ID3D12Resource> = None;
    // SAFETY: plain creation from descriptions that live through the call.
    unsafe { device.CreateCommittedResource(&heap, D3D12_HEAP_FLAG_NONE, &desc, D3D12_RESOURCE_STATE_COMMON, None, &mut made)? };
    made.ok_or_else(windows::core::Error::empty)
}

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

impl Fx {
    fn new(host: &Host, in_size: (u32, u32), format: DXGI_FORMAT) -> windows::core::Result<Fx> {
        use windows::core::s;
        let device = &host.device;
        let out_size = crate::dxgi::output_size().unwrap_or(in_size);
        let motion_cs = super::compile(SOURCE, s!("motionMain"), s!("cs_5_1"))?;
        let resolve_cs = super::compile(SOURCE, s!("resolveMain"), s!("cs_5_1"))?;
        let code = |bytes: &[u8]| D3D12_SHADER_BYTECODE {
            pShaderBytecode: bytes.as_ptr().cast(),
            BytecodeLength: bytes.len(),
        };
        // SAFETY: plain object creation; the descriptions and the shaders'
        // bytes live through each call, and the root signature is inside
        // the shaders.
        unsafe {
            let root: ID3D12RootSignature = device.CreateRootSignature(0, &resolve_cs[..])?;
            let pso = |cs: &[u8]| {
                device.CreateComputePipelineState::<ID3D12PipelineState>(&D3D12_COMPUTE_PIPELINE_STATE_DESC {
                    pRootSignature: std::mem::transmute_copy(&root),
                    CS: code(cs),
                    ..Default::default()
                })
            };
            let motion_pso = pso(&motion_cs[..])?;
            let resolve_pso = pso(&resolve_cs[..])?;
            let kind = D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV;
            let views: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: kind,
                NumDescriptors: GENERATIONS * RING as u32 * 4,
                Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
                NodeMask: 0,
            })?;
            let out = texture(device, out_size, swap_format(format).unwrap_or(format), D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS)?;
            let heap = D3D12_HEAP_PROPERTIES {
                Type: D3D12_HEAP_TYPE_DEFAULT,
                ..Default::default()
            };
            let buffer = D3D12_RESOURCE_DESC {
                Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                Alignment: 0,
                Width: u64::from(BLOCKS) * 8,
                Height: 1,
                DepthOrArraySize: 1,
                MipLevels: 1,
                Format: DXGI_FORMAT_UNKNOWN,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                Flags: D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS,
            };
            let mut motion: Option<ID3D12Resource> = None;
            device.CreateCommittedResource(&heap, D3D12_HEAP_FLAG_NONE, &buffer, D3D12_RESOURCE_STATE_COMMON, None, &mut motion)?;
            let motion = motion.ok_or_else(windows::core::Error::empty)?;
            let queue: ID3D12CommandQueue = device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
                Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
                ..Default::default()
            })?;
            let ready: ID3D12Fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;
            let done: ID3D12Fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;
            let event = CreateEventW(None, false, false, None)?;
            let mut lists = Vec::with_capacity(LISTS);
            for _ in 0..LISTS {
                let allocator: ID3D12CommandAllocator = device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
                let list: ID3D12GraphicsCommandList = device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None)?;
                list.Close()?;
                lists.push((allocator, list, 0));
            }
            // The swap chain last: once it is made, the window is ours.
            let hwnd = match title_window() {
                Some(hwnd) => hwnd,
                None => window(out_size.0, out_size.1)?,
            };
            let factory: IDXGIFactory2 = CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))?;
            let desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: out_size.0,
                Height: out_size.1,
                Format: swap_format(format).unwrap_or(format),
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: BUFFERS,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                ..Default::default()
            };
            let swap_chain = factory.CreateSwapChainForHwnd(&queue, hwnd, &desc, None, None)?.cast::<IDXGISwapChain3>()?;
            tracing::info!(
                in_width = in_size.0,
                in_height = in_size.1,
                out_width = out_size.0,
                out_height = out_size.1,
                render_scale = crate::dxgi::render_scale(),
                framegen = framegen(),
                framegen_ms = framegen_ms(),
                sharpen = sharpen(),
                "presenting through our passes (VICTORY_RENDER_SCALE: the title renders smaller and is scaled up; VICTORY_FRAMEGEN=1: frames in between)"
            );
            let mut fx = Fx {
                queue,
                swap_chain,
                hwnd,
                out_size,
                root,
                motion_pso,
                resolve_pso,
                view_cpu: views.GetCPUDescriptorHandleForHeapStart().ptr,
                view_gpu: views.GetGPUDescriptorHandleForHeapStart().ptr,
                step: device.GetDescriptorHandleIncrementSize(kind),
                views,
                out,
                motion,
                ring: Vec::new(),
                in_size: (0, 0),
                in_format: DXGI_FORMAT_UNKNOWN,
                generation: 0,
                retired: Vec::new(),
                used: [0; RING],
                copies: Vec::new(),
                copy_now: None,
                ready,
                ready_value: 0,
                done,
                done_value: 0,
                lists,
                event,
                frames: 0,
                last_real: 0,
                shown: None,
                last_present: None,
                interval: 0.0,
                logged: std::time::Instant::now(),
                meter: Meter::new(),
            };
            fx.make_ring(device, in_size, format)?;
            Ok(fx)
        }
    }

    /// The ring for frames of `size` in `format`, and its descriptor sets.
    fn make_ring(&mut self, device: &ID3D12Device, size: (u32, u32), format: DXGI_FORMAT) -> windows::core::Result<()> {
        let typed = swap_format(format).unwrap_or(format);
        let mut ring = Vec::with_capacity(RING);
        for _ in 0..RING {
            ring.push(texture(device, size, typed, D3D12_RESOURCE_FLAG_NONE)?);
        }
        self.retired.append(&mut self.ring);
        self.ring = ring;
        self.in_size = size;
        self.in_format = format;
        self.generation = (self.generation + 1) % GENERATIONS;
        self.frames = 0;
        let uav = D3D12_UNORDERED_ACCESS_VIEW_DESC {
            Format: DXGI_FORMAT_UNKNOWN,
            ViewDimension: D3D12_UAV_DIMENSION_BUFFER,
            Anonymous: D3D12_UNORDERED_ACCESS_VIEW_DESC_0 {
                Buffer: D3D12_BUFFER_UAV {
                    FirstElement: 0,
                    NumElements: BLOCKS,
                    StructureByteStride: 8,
                    CounterOffsetInBytes: 0,
                    Flags: D3D12_BUFFER_UAV_FLAG_NONE,
                },
            },
        };
        for k in 0..RING {
            let at = |n: usize| D3D12_CPU_DESCRIPTOR_HANDLE {
                ptr: self.view_cpu + (self.set(k) * 4 + n as u32) as usize * self.step as usize,
            };
            // SAFETY: slots of our heap that no list of ours still reads
            // (a generation is used again only after 15 more sizes); live
            // resources.
            unsafe {
                device.CreateShaderResourceView(&self.ring[k], None, at(0));
                device.CreateShaderResourceView(&self.ring[(k + RING - 1) % RING], None, at(1));
                device.CreateUnorderedAccessView(&self.out, None::<&ID3D12Resource>, None, at(2));
                device.CreateUnorderedAccessView(&self.motion, None::<&ID3D12Resource>, Some(&uav), at(3));
            }
        }
        tracing::info!(width = size.0, height = size.1, format = format.0, "frames of the title taken for our passes at this size");
        Ok(())
    }

    /// The descriptor set of ring texture `k` in this generation.
    fn set(&self, k: usize) -> u32 {
        self.generation * RING as u32 + k as u32
    }

    /// Records our passes for ring texture `cur` into `l`, shown into the
    /// back buffer `back`: the motion field first if `motion`, and the
    /// frame moved on by `amount` of it (0: as it is).
    ///
    /// # Safety
    /// `l` is open; `back` is the swap chain's current back buffer.
    unsafe fn record(&self, l: &ID3D12GraphicsCommandList, back: &ID3D12Resource, cur: usize, motion: bool, amount: f32) {
        let prev = (cur + RING - 1) % RING;
        let (bx, by) = (self.in_size.0.div_ceil(16), self.in_size.1.div_ceil(16));
        let constants: [u32; 12] = [
            self.in_size.0,
            self.in_size.1,
            self.out_size.0,
            self.out_size.1,
            amount.to_bits(),
            (if self.in_size == self.out_size { 0.0f32 } else { sharpen() }).to_bits(),
            u32::from(amount > 0.0),
            bx,
            by,
            0,
            0,
            0,
        ];
        let table = D3D12_GPU_DESCRIPTOR_HANDLE {
            ptr: self.view_gpu + u64::from(self.set(cur) * 4) * u64::from(self.step),
        };
        let read = D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE;
        let common = D3D12_RESOURCE_STATE_COMMON;
        let uav = D3D12_RESOURCE_STATE_UNORDERED_ACCESS;
        // SAFETY: (the caller's) an open list; our own live objects.
        unsafe {
            l.SetDescriptorHeaps(&[Some(self.views.clone())]);
            l.SetComputeRootSignature(&self.root);
            l.ResourceBarrier(&[
                transition(&self.ring[cur], common, read),
                transition(&self.ring[prev], common, read),
                transition(&self.out, common, uav),
                transition(&self.motion, common, uav),
            ]);
            l.SetComputeRootDescriptorTable(1, table);
            l.SetComputeRoot32BitConstants(0, 12, constants.as_ptr().cast(), 0);
            if motion && bx * by <= BLOCKS {
                l.SetPipelineState(&self.motion_pso);
                l.Dispatch(bx, by, 1);
                l.ResourceBarrier(&[uav_barrier(&self.motion)]);
            }
            l.SetPipelineState(&self.resolve_pso);
            l.Dispatch(self.out_size.0.div_ceil(8), self.out_size.1.div_ceil(8), 1);
            l.ResourceBarrier(&[
                transition(&self.out, uav, D3D12_RESOURCE_STATE_COPY_SOURCE),
                transition(back, D3D12_RESOURCE_STATE_PRESENT, D3D12_RESOURCE_STATE_COPY_DEST),
                transition(&self.ring[cur], read, common),
                transition(&self.ring[prev], read, common),
                transition(&self.motion, uav, common),
            ]);
            l.CopyResource(back, &self.out);
            l.ResourceBarrier(&[
                transition(&self.out, D3D12_RESOURCE_STATE_COPY_SOURCE, common),
                transition(back, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_PRESENT),
            ]);
        }
    }

    /// Runs our passes for ring texture `cur` on our queue (after the
    /// host's queue reached `ready` value `wait`, if given) and presents.
    fn show(&mut self, cur: usize, motion: bool, amount: f32, wait: Option<u64>) -> windows::core::Result<()> {
        // SAFETY: our own objects; a list is reset only once our queue is
        // done with it; the back buffer is the current one, under the lock.
        unsafe {
            let done = self.done.GetCompletedValue();
            let i = match self.lists.iter().position(|l| l.2 <= done) {
                Some(i) => i,
                None => {
                    let (i, oldest) = self.lists.iter().enumerate().min_by_key(|(_, l)| l.2).map(|(i, l)| (i, l.2)).unwrap_or((0, 0));
                    self.done.SetEventOnCompletion(oldest, self.event)?;
                    WaitForSingleObject(self.event, INFINITE);
                    i
                }
            };
            let (allocator, list) = (self.lists[i].0.clone(), self.lists[i].1.clone());
            allocator.Reset()?;
            list.Reset(&allocator, None)?;
            let index = self.swap_chain.GetCurrentBackBufferIndex();
            let back: ID3D12Resource = self.swap_chain.GetBuffer(index)?;
            self.record(&list, &back, cur, motion, amount);
            list.Close()?;
            if let Some(value) = wait {
                self.queue.Wait(&self.ready, value)?;
            }
            self.queue.ExecuteCommandLists(&[Some(list.cast::<ID3D12CommandList>()?)]);
            self.done_value += 1;
            self.queue.Signal(&self.done, self.done_value)?;
            self.lists[i].2 = self.done_value;
            self.used[cur] = self.done_value;
            self.used[(cur + RING - 1) % RING] = self.done_value;
            let _ = self.swap_chain.Present(0, DXGI_PRESENT(0));
        }
        Ok(())
    }

    /// The title's frame `texture` (in host state `state`): copied into
    /// the ring on the host's queue, then shown through our passes.
    fn present(&mut self, host: &Host, texture: &ID3D12Resource, state: u32) -> windows::core::Result<()> {
        // SAFETY: a live resource.
        let desc = unsafe { texture.GetDesc() };
        let size = (desc.Width as u32, desc.Height);
        if (size, desc.Format) != (self.in_size, self.in_format) {
            self.make_ring(&host.device, size, desc.Format)?;
        }
        let slot = (self.frames % RING as u64) as usize;
        let state = D3D12_RESOURCE_STATES(state as i32);
        let ahead = late_free_present();
        let index = if ahead {
            let finished = FINISHED.load(Ordering::Acquire);
            match self.copies.iter().position(|(_, flown)| *flown <= finished) {
                Some(i) => Some(i),
                None => {
                    let list = host.new_list()?;
                    list.close();
                    self.copies.push((list, 0));
                    Some(self.copies.len() - 1)
                }
            }
        } else {
            None
        };
        if index.is_none() && self.copy_now.is_none() {
            let list = host.new_list()?;
            list.close();
            self.copy_now = Some(list);
        }
        let copy = match index {
            Some(i) => &self.copies[i].0,
            None => self.copy_now.as_ref().ok_or_else(windows::core::Error::empty)?,
        };
        copy.reset();
        let ring = &self.ring[slot];
        // SAFETY: an open list of ours; live resources; the ring texture
        // rests in the common state between our uses.
        unsafe {
            let list = &copy.list;
            list.ResourceBarrier(&[
                transition(texture, state, D3D12_RESOURCE_STATE_COPY_SOURCE),
                transition(ring, D3D12_RESOURCE_STATE_COMMON, D3D12_RESOURCE_STATE_COPY_DEST),
            ]);
            list.CopyResource(ring, texture);
            list.ResourceBarrier(&[
                transition(texture, D3D12_RESOURCE_STATE_COPY_SOURCE, state),
                transition(ring, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_COMMON),
            ]);
        }
        copy.close();
        // Our queue is done with the ring texture before it is written.
        // SAFETY: our fence; a wait on the host's queue.
        unsafe { host.queue.Wait(&self.done, self.used[slot])? };
        match index {
            Some(i) => {
                let flown = host.execute_own(&self.copies[i].0);
                self.copies[i].1 = flown;
            }
            None => {
                if let Some(copy) = &self.copy_now {
                    host.execute(&[copy]);
                }
            }
        }
        self.ready_value += 1;
        // SAFETY: our fence, behind the copy on the host's queue.
        unsafe { host.queue.Signal(&self.ready, self.ready_value)? };
        let motion = framegen() && self.frames >= 1;
        self.frames += 1;
        self.show(slot, motion, 0.0, Some(self.ready_value))?;
        self.last_real = self.done_value;
        self.shown = None;
        let now = std::time::Instant::now();
        if let Some(last) = self.last_present {
            let dt = now.duration_since(last).as_secs_f64().min(1.0);
            self.interval = if self.interval <= 0.0 { dt } else { 0.7 * self.interval + 0.3 * dt };
        }
        self.last_present = Some(now);
        self.meter.frame(self.hwnd);
        if framegen() && self.logged.elapsed().as_secs() >= 5 {
            let made = MADE.swap(0, Ordering::Relaxed) as f64 / self.logged.elapsed().as_secs_f64();
            self.logged = now;
            tracing::info!(
                "frames made between the title's (VICTORY_FRAMEGEN=1): {made:.1} a second, the title's every {:.0} ms",
                self.interval * 1000.0
            );
        }
        Ok(())
    }

    /// The thread's turn: a frame moved on from the title's last, if that
    /// one is on the screen and the title's next is not due yet.
    fn between(&mut self) {
        if self.frames < 2 || self.interval <= 0.0 {
            return;
        }
        // SAFETY: a plain query.
        if unsafe { self.done.GetCompletedValue() } < self.last_real {
            return;
        }
        let shown = *self.shown.get_or_insert_with(std::time::Instant::now);
        let amount = shown.elapsed().as_secs_f64() / self.interval;
        if !(0.1..=1.0).contains(&amount) {
            return;
        }
        let cur = ((self.frames - 1) % RING as u64) as usize;
        match self.show(cur, false, amount as f32, None) {
            Ok(()) => {
                MADE.fetch_add(1, Ordering::Relaxed);
            }
            Err(e) => tracing::warn!(error = %e, "a frame in between was not shown"),
        }
    }
}

/// The thread that shows frames in between ([`framegen`]).
fn between_thread() {
    loop {
        std::thread::sleep(std::time::Duration::from_millis(framegen_ms()));
        let mut fx = FX.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(fx) = fx.as_mut() {
            fx.between();
        }
    }
}

/// Shows the title's frame `texture` through our passes. `None`: not
/// here (a format the passes do not write, or setting up failed), the
/// caller presents as before.
pub(super) fn present(host: &Host, texture: &ID3D12Resource, state: u32) -> Option<Option<bool>> {
    // SAFETY: a live resource.
    let desc = unsafe { texture.GetDesc() };
    let format = swap_format(desc.Format)?;
    if format != DXGI_FORMAT_R8G8B8A8_UNORM && format != DXGI_FORMAT_R10G10B10A2_UNORM && format != DXGI_FORMAT_R16G16B16A16_FLOAT {
        return None;
    }
    let mut fx = FX.lock().unwrap_or_else(|e| e.into_inner());
    if fx.is_none() {
        match Fx::new(host, (desc.Width as u32, desc.Height), desc.Format) {
            Ok(made) => {
                *fx = Some(made);
                if framegen() {
                    let _ = std::thread::Builder::new().name("victory-frames-between".into()).spawn(between_thread);
                }
            }
            Err(e) => {
                FAILED.store(true, Ordering::Relaxed);
                tracing::warn!(error = %e, "our present passes could not be set up: frames are shown as before");
                return None;
            }
        }
    }
    let fx = fx.as_mut()?;
    if let Err(e) = fx.present(host, texture, state) {
        tracing::warn!(error = %e, "a frame was not shown through our passes");
    }
    Some(Some(false))
}
