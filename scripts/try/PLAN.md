# VIctory Try: plan and to-do (6.10, updated for 6.16)

Goal: the whole picture rendered, **20+ fps** on the reporter's PC: RTX 4050
Laptop (5921 MB, budget about 5153 MB), 12 threads, about 16 GB of RAM,
Series S profile, game 1.0.2.1.

**Base: this fork's own engine, always.** (6.9 made AvatarManz's engine the
default; the owner turned that down: the base never changes again.) The
other forks' most important fixes are ported into our engine one at a time,
each with its author named, each with the log line that shows it working.
avm/8GB stay selectable with N for comparing only.

How the loop works: a version ships, the user runs it and sends the report
zip, we read it, fix, ship the next. Every round PortWatch and the forks
below are checked again.

## Where everyone stands (2026-11-26, PortWatch + their READMEs)

| Fork | Card | fps | What gets them there |
|---|---|---|---|
| Belobug (`a-publier`) | RX 9070 XT | ~22 | avm c9c932c + prelude uploads (flicker), VRAM-aware texture cap, **PC FSR via the host DLL** |
| avatarmanz/VIctory-7900XTX | RX 7900 XTX | 16 (45 ms Series S) | renderer ports 1-7 (CBV -4.7 ms, guard -2, journal -1..-3), THIN list calls on 8 threads, copy-queue textures + show-after, shaders/pipelines made ahead (`warm`) |
| DexNom/VIctory | RX 7900 XTX | 3-4 | best picture: ray tracing, cube gradients, the PC FSR bridge (ab34a8b2) |
| kelvaroq (stellarr-studio) | RTX 5080 | ~10 (11 on a 4090 for 36 min) | patches 0001-0064 on fork f85101b (an ancestor of ours): wide card hashes + mesh launch (0041, 198 -> 120 ms), async submit + async present + copy queues by default (0040/0048), translate-once descriptors (0055), unified memory layer (0063, `VICTORY_UMA`, not in a build yet) |
| DoubleGhosty/cool_fox | - | - | community continuation (codec_d): renderer ports 1/5/6/7 by default, coalesced card compares (Xyzstk f2dd4508), frame-rate and render-scale switches |
| this fork | RTX 4050L / RTX 4060 | 1.6-3.3 (6.7) / 4.2-4.7 (6.12, MAX) | 6.12: relay waits 130 ms a frame for fetches queued behind the flights (copy-engine fetch off since an old lost card), GPU ~150 ms with no barrier trimmed |

## Done in 6.10

- [x] Own engine is the default again (one-time switch back; N: own -> avm -> 8GB).
- [x] **PC FidelityFX upscaler** (Belobug cd8dfc06 from DexNom ab34a8b2):
      `victory-hle/modules/fidelityfx.rs` serves the title's
      `amd_fidelityfx_loader_xbox.dll` with AMD's signed
      `amd_fidelityfx_upscaler_dx12.dll` on the host GPU. Changed for us:
      any profile (Series S 1280x720 -> 1920x1080 too, not only their 4K
      patch), provider chosen FSR 4.1.2 -> newest other (FSR 3.1 on NVIDIA),
      the next provider tried if the card refuses one, and a dispatch the PC
      DLL cannot record falls back to our temporal resolve (never black).
      `victory-gpu/external.rs` records it on our plain host list. Try
      downloads the DLL once (57 MB, AMD's GitHub, signature checked).
      Expected: no blocky lit/reflecting surfaces, sharp hair, real jitter
      accumulation. T: FSR / own / off.
- [x] Own temporal resolve (fallback, T=own): Catmull-Rom colour sample,
      sharpening against a smooth tent blur (the old 3x3 mean of the nearest
      rendered texel drew the 720p grid into bright surfaces), sharpen 10 %.
- [x] avm's native shader reader: shaders stored without XBCH (kind 0x0c,
      avm e70e031c) and the FSPX/ACCC layout (AMD-compiled shaders, avm
      66afb2b4; parser only so far).
- [x] Logs: `upscaler PC FidelityFX ...` line at start, providers offered
      and chosen, every 600 upscales how many were jittered (both paths).

## Done in 6.16 ("room for windows")

Found in the 6.15 report (RTX 4050 Laptop, MAX, low preset, 2.4-3.4 fps in
the world, 30-36 in the menu): VRAM 5,048-5,102 of 5,153 MB all through the
world (`TEXTURE_ROOM_PERCENT=99`), windows: BAR 117 (436 MB), card 0 (768
turned down for room), so most windows were read across the bus; card 58-72 %
busy. Texture rush toggled 209 times (111,497 uploads, 51,988 MB, 75,957
placeholders shown, BC4 white / BC5 128,128,255 = the face flicker). Disk: a
new shader translation set per build (~1 GB), target never collected, other
engines' trees, logs never pruned.

- [x] `shadow::windows_room_kept` counted in `resource::room`; `window_line`
      96 % (80 off); `windows_line` in the stats (BAR / card / PC MB).
- [x] Rush: `rush_next` with a gap (`VICTORY_TEXTURE_RUSH_GAP_S`, 30);
      `descriptor::host::unrushing`: an unrushed texture's detile leaves out
      no level.
- [x] Shader cache per build folder, `prune` (`VICTORY_SHADER_CACHE_KEEP`).
- [x] Card meter without the frame log; kinds line; table at least every
      60 s. Try sets `VICTORY_CARD_METER=1` (config `meter=off`).
- [x] Try: `Tidy-Disk` / `Tidy-Target` / `TIDY`; MAX on small cards 94 %.
- Next, from the next report: the submit thread's 55-169 ms a frame
  (descriptor slots, heap-buffers 47-78 ms, heap worker 88-94 ms, monitorx
  450k exceptions/s); keep the last real view instead of the placeholder.

## Done in 6.15 ("new things fast")

Found in the 6.12 report: the library saves (1.1 GB each, ten in eighteen
minutes, every 20-40 s with memory Normal) line up with the worst frames:
20:27:55 397 ms + 19 draws/frame dropped "being made", 20:28:41 523 ms + 24.3
dropped, then the 5196 ms frame at 20:28:50. `StorePipeline` waited for the
save's write lock, so every pipeline finished during a save waited too. And
`VICTORY_STAND_IN=0` (new38 off) dropped every draw of a pipeline being made.

- [x] Library: store later (cool_fox 4a028f31, Esteban Viera; on here,
      `VICTORY_LIBRARY_STORE_LATER=0` goes back); one save at a time; saves on
      a thread of their own (`save_soon`, `VICTORY_LIBRARY_SAVE_THREAD=0`);
      in the world every `VICTORY_LIBRARY_SAVE_S` (300) whatever the memory.
      Save line: `handed_out_during_saves`.
- [x] Stand-ins on their own Try switch (config `standin`), and
      `VICTORY_STAND_IN_FIRST` (engine, on): a same-shape stand-in before any
      wait (Kelvaroq 0050). Try: `VICTORY_PIPELINE_WAIT_MS=250`. Log: the
      stand-in counts line now has "at once, before any wait".
- [x] A panicking pipeline build is refused (cool_fox 744c09ba).
- [x] `alias_of` holds the aliased texture (cool_fox 94f35b7f).
- Looked at, not taken: cool_fox 6ad1eff1 (sparse mips on address-backed
  textures: our tiles path differs, `tiles::by_quality`), f4164f86 (skips all
  hair strand passes), c9f654e0 (already here), f2dd4508/8eb4f320 (their tree).

## Why 3-4 fps (6.12 report, RTX 4060 8 GB, 32 GB, MAX on) and 6.14

Steady state 20:36-20:42: 4.2-4.7 fps, 215-240 ms a frame, ~3,500 draws.

| part of a frame | ms | cause |
|---|---|---|
| relay thread waiting for fetches of card copies' pieces | **129-133** | `VICTORY_COPY_FETCH=0`: the fetch went on the direct queue behind every flight sent since; an old lost card had set config `new38=off`, which also turned off copy fetch and barrier trimming for good |
| card busy (our count 66-68 %, Task Manager 3D 87-88 %) | ~150 | `VICTORY_TRIM_BARRIERS=0` (0 trimmed), 105 hash passes + 58 compares a frame, one thread a piece |
| preparing on the title's thread (copies 24, refreshes 24, descriptor mirror 21-24) | 50-55 | the title's thread prepares every submission itself, then waits in the present |
| title waits for the copy-back to hear a fence | (inside the above) | `VICTORY_LAZY_BACK=0` (an old access-violation crash) |
| windows outside buffers | 0 MB | so the 6.13 BAR windows could not help this PC |

MAX lifted caps; none of these is a cap. The Series S has one memory: the
GPU's writes are simply there for the CPU (no fetch, no compare, no
copy-back), and the game records frame N+1 while the GPU draws frame N
(the present returns at once). 6.14 moves us toward that:

- [x] Copy fetch and barrier trimming on their own switches (config
      `copyfetch`, `trim`), on; a one-time migration (`mig613`) undoes the
      old lock-out. Last in the lost-card chain.
- [x] Lazy copy-back on again once (migration); an AV crash turns it off.
- [x] `VICTORY_ASYNC_SUBMIT=1` + `VICTORY_ASYNC_PRESENT=1` (config `ahead`):
      one frame ahead (Kelvaroq 0040/0048 defaults). Second in the lost-card
      chain after wide hashes.
- [x] `VICTORY_CARD_HASH_WIDE=1` (config `widehash`; fork #1014 martindevbr,
      Kelvaroq 0041 default). First in the lost-card chain after BAR windows.
- [x] Joined relay fetch copies (Kelvaroq 0032; `VICTORY_RELAY_COPY_RUNS=0`
      goes back), engine default.

Expected in the 6.14 report: relay `waited ... for fetches` a few ms (not
130), `what shaders wrote ... fetched on the card's copy engine` at start,
`barriers and full waits left out so far` > 0, `submissions are prepared and
sent by a thread of ours`, `presents go behind the queue's line`, the lazy
line. Frame then bounded by the card (~100 ms?) -> about 8-10 fps; the next
step is the card's own time (our passes, indirect clamps, 354 spread
dispatches a frame) and the descriptor mirror.

Not ported (yet): Kelvaroq 0041 mesh launch stage (1,389 lines), 0055
translate-once descriptors (1,012), 0063 UMA (1,988, not in their build);
cool_fox f2dd4508 coalesced compares (on their tree's `CardRange`), 8a31b96e
journal (1,503 lines).

## Done in 6.13 ("NVIDIA fixes"; deep search of the NVIDIA forks)

Searched: trustno1-coder/VIctory-NVIDIA (`nv/main` 6a31e431), edsuuu/VI-BR-PORT
(`br/main` 2aa1895d), Itsbatt/victory-unified, TaynanGT/Vice-Tay. Ported:

- [x] **Memory windows through Resizable BAR** (nv cda4987a + 85a529b6,
      Indy2l): `VICTORY_WINDOWS_BAR_MB` in `shadow.rs` (`window_bar_most`,
      `window_bar_for`): windows outside buffers are one buffer in a GPU
      upload heap, no copy in the PC's memory, no list of changes; own
      counter, not tied to `VICTORY_CARD_WINDOWS_MB`; off with
      `VICTORY_GPU_UPLOAD=0`; refused past 80% of the card's budget.
      Measured there: 16 GB RTX 3060 PC, `0x8007000E` and the driver fault
      gone, shared memory 4.4 GB instead of 7.5, 1.6-3 -> 10-12 fps. Try sets
      it on PCs under 24 GB of RAM: 20% of the card under 8 GB (about
      1184 MB on the 4050), 30% above, at most 3072; MAX 35%. A lost card
      (with the BAR line in the log) turns it off next (config `barwin`).
      Log: `windows outside buffers in the card's memory through a resizable
      BAR` (supported=true?) and `windows through the BAR (...)`.
- [x] **Texture read-backs on their own thread** (nv 2d1dc382, Indy2l):
      `texback::land` only copies the texels out; the lock wait, tiling and
      write go to the `texture read-backs` thread, newest copy only. There:
      124-416 ms landing stalls while the world streamed on 16 GB.
      `VICTORY_TEXBACK_ASYNC=0` goes back. Log: `texture readback landed`
      now carries waited_ms / worked_ms / replaced.
- [x] **A slow pipeline is waited for in one frame only**
      (`VICTORY_PIPELINE_WAIT_ONCE`, on; br dc3fa30b, nv 77e13a97): no more
      half-second freezes every frame while a driver compile runs.
- [x] **Compute pipelines start at creation** (br b9d4f96c;
      `VICTORY_PIPELINE_EARLY_COMPUTE=0` goes back), only with workers.
- [x] **Decoration bake found by name** (br 10ac68f9): `PS_DecorationBlend_*`
      and `PS_BakeDecorations_*` get their sources uploaded first (cutscene
      faces came out black).
- [x] **Texture change lists compacted, not cleared** (br 87dc3075 +
      47fa23c9, myokex): 2^18 ranges, a full list joins neighbours instead
      of forgetting streamed textures.
- [x] **Card compare submitted on its own** (nv d1fe0bd0, Jeremy Goldberg):
      D3D12 error 538 (buffers decay to COMMON between ExecuteCommandLists).
- [x] (6.12 open question) `$Src` is defined in try.ps1 (line 53), so the
      rt-off list path resolves.

Looked at and left (and why):
- br a46bbdc6 + 689be4d2 (wave64 programs take the wave-wide form on
  32-lane hosts): our wide form lost the card in runs 121-124; needs the
  295-line barrier fix ported and tested first. Next round.
- br fd36f4a4 (movie planes per-list upload): intro video steps back only;
  later.
- br 44ead3c5 SHOW_AFTER default: our descriptor host has no show-after yet.
- br 389b9747 (1 MiB / 1 GiB card buffers on 8-12 GB): our 6 GB card was at
  4822/5153 MB; MAX already raises it. Try after the 6.13 report.
- br 5c17c8c8 (#1241 alias source): already in our tree.
- br 72903b29 (64 MB pipeline worker stack): already in our tree.
- nv c05b44b3 (ambient depth SRV): our ambient pass differs; check if the
  debug layer shows error 646.
- nv 12b1d16d (HLE mouse): if the user wants mouse camera.

## Done in 6.12 ("all-out")

The user: "still bad"; asked for a mode with no caps that reads the PC and
gives the game all but ~1%, and a deep search of git.gtahub.st/explore/repos.

- [x] **All-out mode** (MAX, `VICTORY_TRY_ALL_OUT=1`; try.ps1 `Set-AllOut`),
      from `Win32_OperatingSystem` memory, `ProcessorCount`, the card's
      memory: threads-1 for `VICTORY_WIDE_THREADS`, `VICTORY_DETILE_THREADS`
      and the new `VICTORY_PIPELINE_WORKERS`; `VICTORY_PRIORITY=high` and
      `VICTORY_WORKING_SET_PERCENT=90` (new, `victory-hle/src/runtime.rs`);
      `VICTORY_TEXTURE_ROOM_PERCENT=99` (clamp raised 97 -> 99), no
      `TEXTURE_BUDGET_MB`, `TEXTURE_STAGE_MB=512`, `STAGING_POOL_MB=256`,
      `DSTORAGE_STAGING_MB=256` (cool_fox via CGBJ), `CARD_BUFFERS_MB` a fifth
      of the card (1 GB at least), `MERGE_UNWATCHED_MB=256`; the pressure
      guard's limits from the new `VICTORY_PRESSURE_RAM_MB`,
      `VICTORY_PRESSURE_COMMIT_MB`, `VICTORY_PRESSURE_CARD_PERCENT` (each
      `mild,critical`; `pressure.rs` `Limits`) at 2%/1% of memory and
      98/99% of the card. First thing turned off after a lost card.
      Left as they are on purpose: `SHADER_LOOP_LIMIT` (1024 lost the card),
      `CARD_ROOM_FOR_TARGETS_MB` (room the render targets need),
      `LATE_WAIT_LISTS` (the 6.7 report never waited on it),
      `PIPELINE_HEAVY` (more than 3 heavy compiles ran the driver out of
      memory). Honest limit: in the 6.7 report the PC had 368-740 MB free
      and 1.2-1.6 GB left to commit, the card 4.8 of its 5.15 GB budget;
      there is no spare memory to give, only the guard to move. Log:
      `memory pressure starts later`, `the whole machine for the title ...
      high=true`, `pipeline workers workers=11`.
- [x] **Ray-only passes** (edsuuu/VI-BR-PORT ab4571df, 1556de82, f1e5e454,
      24f8d2ed): `pipeline.rs` `for_rays` by entry-point name
      (`CSComputeSkinning*`, `CSGenerateHairVB`, `CS_RaytracingResolve*`,
      `CS_ScreenProbes_ActivateRays*`, `..._TraceRays*_BVH*`,
      `CS_RaytraceVBuffer*` but not Voxels/Fallback); with
      `VICTORY_SKIP_RT=1` and the builder off their dispatches are left out
      (`draw.rs`) and, ours on top, they are never built (`make.rs`,
      `recipe.rs` prepare). There: 6.8 -> 10.2 fps (skinning), +8 % (traces).
      Switches `VICTORY_RT_SKIN/RESOLVE/TRACE`, `VICTORY_ACTIVATE_RAYS`.
      Log: `passes that only serve rays are left out`, `... not built`.
- [x] **rt-off list** (trustno1-coder/VIctory-NVIDIA 77e13a97):
      `scripts/try/rt-off-hashes.txt` (1,000 resolve + 34 traced shadows)
      as `VICTORY_DENY_PIPELINES` with B (our release build has no
      `VICTORY_SKIP_PIPELINES`; deny works in release). Denied pipelines are
      skipped by prepare too; the log says the first 8, then powers of two.
- [x] **NaN scrub** (same commit): `emit/lower.rs` `scrub_nan` on pixel
      colour exports, `VICTORY_NAN_SCRUB=1` on in Try (not in plain mode).
- [x] Searched (2026-11-26): gtahub explore, `q=victory` by stars. Fetched
      `nv` (trustno1-coder/VIctory-NVIDIA), `bat` (Itsbatt/victory-unified),
      `br` (edsuuu/VI-BR-PORT), `tay` (TaynanGT/Vice-Tay). cesarzxk's CGBJ,
      N137S and codec-d/cool_fox are not on GitHub (gtahub only).

Next candidates (not in 6.12):
- nv a9632bc8 `VICTORY_DYNAMIC_SLOTS` on a capped heap: the 6.7 report
  refused no tables (no "no free dynamic descriptor slots"), so not needed
  yet; needed if lists in the air go past 384.
- nv `VICTORY_PIPELINE_WAIT_ONCE` with `PIPELINE_WAIT_MS=10000` (no dropped
  draws for pipelines being made, one wait each).
- nv `VICTORY_AMBIENT_AIR`/`AMBIENT_MOST` (their ambient is the estimate
  kind; ours is the constant: br 557d2f03 makes the air bound work with
  the constant too).
- br a06da66f (tiled cascade shadows exact on wave32), 5c17c8c8 (fork PR
  #1241, card-written texture views), 389b9747/ab9ae781 (card copies of
  buffers from 64 KB on 8-12 GB cards).
- bat 2a95f8b5 (RTX 3070 paired bitcode, descriptor/shadow hardening).

## Done in 6.11

- [x] **Shader stutter, disk side**: `pipeline/library.rs` `prefetch()`
      wanted the library + 4 GB free; the 6.7 report had 4295 MB free for a
      773 MB library, so nothing was read ahead and each pipeline loaded in
      play paged in from disk (2.6-4.5 s each in the 6.2 report). Now 1 GB
      room (`VICTORY_LIBRARY_PREFETCH_ROOM_MB`), partial read of the front
      when the whole does not fit. Log: `host pipeline library read ahead`
      (and `... in part`).
- [x] **Shader stutter, translation side**: Try runs `victory prepare
      <game folder>` once per build: every shader container translated into
      the shader cache (avm's `victory shaders` idea, with our cache keys).
      Log: `translating N shaders ...`, `translated ... from the files`.
- [x] `VICTORY_JOIN_PARTS=1` in the Y set (viktoschi, leonida launcher).
- [x] leonida c6c9da9f, first part: `WIDE_JOB` 256 KiB.
- [x] avm 011735ff: anisotropic point-mip samplers (static, console and
      host-layout samplers, through `sampler::enhance`).
- [x] CI is green for the first time on this branch. The xtask policy test
      had failed on every run: CI checks the pull request merged onto
      `main`, where the owner's `VIctory-report-*.zip` uploads sit (now let
      through, top level only), and on `host.rs` (587 KB) / `lower.rs`
      (545 KB) over 512 KiB (Rust sources may be 1 MiB). CI's step outputs
      moved out of the checkout; policy violations show in the annotations.
- [x] try.ps1: the 8 GB engine's fallback wrote `engine = 'ours'` (no such
      name): `own`.

Looked at and **not** ported this round (why):
- leonida 8f1e9032 (dark-scene ambient) and avm 9576c45/b4ba4bb (ambient
  settle): our `ambient.rs` is a different implementation that already
  follows the scene's brightness (6.6 `VICTORY_AMBIENT_FOLLOW`).
- Belobug e9428969: its pipeline part is a guard for AMD's compiler
  crashing (RX 9070 XT); not needed on NVIDIA.
- mandy-port 9d581cb5 (DirectStorage handle per worker): sits on ffc80295,
  which conflicts with our `dstorage.rs`; start-up time only (~5 s).
- leonida `INPLACE=0`: ours is already off unless H is on.
  `TEXTURE_NO_WAIT`: already the default here.
- avm CBV/guard/journal (`0dec5dc`, `cc761bf`, `3406192`): 500-2000 lines in
  `watch.rs`/`host.rs`/the translator each, off by default even in avm,
  and they conflict everywhere with ours. They save 2-5 ms of a 45 ms
  frame there; our 6.7 frames were 520-820 ms, spent elsewhere (below).
  Waiting for the 6.11 report's numbers before taking that risk.

## Where the time goes (6.7 report, last world minutes)

| part of a frame | ms |
|---|---|
| preparing (copies 47-229 ms, textures 10-40 ms) | 160-330 |
| heap pass, textures picked by index | 64-138 |
| refreshes, all threads (write watch asks 30-65) | 58-169 |
| copying back | 54-102 |
| on the GPU | 12-317 |

Draws dropped: ~2000 a frame are the ray-tracing structure builder (off with
B), about 10 a frame "being made". So in the world, pipelines were *not* the
frame-rate limit; the CPU's copies are. The 6.8+ fx set (heap thread,
parallel refresh) targets rows 2-3; the 6.11 report will show how much.

## To do, in order

### 0. Read the 6.14 report (needs the user's zip)
- [ ] The expected lines above; relay fetch wait; card busy %; fps.
- [ ] Any crash (lazy copy-back) or lost card (which step went off).
- [ ] If the card is now the limit: time our passes (`VICTORY_CARD_HASH_WIDE`
      line), port Kelvaroq 0055 descriptors / 0041 mesh launch next.

### 1. Read the 6.13 report (needs the user's zip)
- [ ] `through a resizable BAR` supported=true? `windows through the BAR`
      counts and MB; memory pressure and `shared with the card` against 6.7
      (368 MB free, Critical); any 0x8007000E or device removed.
- [ ] `texture readback landed` waited_ms against worked_ms; landing line.
- [ ] Pipeline waits per frame (one frame per slow pipeline now).

### 1b. Read the 6.11 report (needs the user's zip)
- [ ] `upscaler` line: PC FidelityFX used? `FidelityFX: upscalers the PC
      build offers` (expect 3.1.x on the 4050), `upscaler context on the host`,
      `recorded the title's temporal dispatch` with jitter not 0.
- [ ] If `did not record ... our temporal resolve instead`: which resource
      or state failed (the `index`/`state` warning) and fix `host_state` /
      `legal_state` for it.
- [ ] fps lines, `memory pressure`, pipelines "being made" over time.
- [ ] `host pipeline library read ahead` present (ok=true, how many ms)?
- [ ] `prepare.log`: how many shaders the game's files had, how long.
- [ ] The five frame parts above, against 6.7, with the heap thread on.

### 2. Picture
- [ ] "PS3 characters": the 6.7 card held only 263 MB of title textures
      (1.1 of 5.1 GB used): not a VRAM cap. Suspects: the title's streamer
      starved by 2-3 fps (fixes itself with speed), LEVELS_ON_DEMAND leaving
      top levels without tiles (check `uploads that left a held level
      without tiles` in the stats), TEXTURE_BUDGET_MB=1536 of the low preset.
      Log to add: per-minute count of textures shown below their top level.
- [ ] "Wrong light reflections": SKIP_RT=1 (boost B) has no ray-traced
      reflections. Measure B off once on the 4050; port avm's light settle
      (`9576c45`) and lightprobe/texlate (`b4ba4bb`).
- [ ] Belobug flicker fix (prelude uploads, 5d6bd841) if the 6.11 report
      or the user shows flicker.
- [ ] Rest of the ACCC port (avm 66afb2b4: dot4, pk_fmac, src_scc, raw
      buffer descriptors, s_bfm samplers) so the title's own console FSR
      translates when the PC DLL is missing.

### 3. Speed to 20+ (largest first)
- [ ] avm renderer ports into our engine: CBV `0dec5dc` (-4.7 ms), guard
      `cc761bf` (-2 ms), journal `3406192` (-1..-3 ms), descriptors `f249fe0`.
- [ ] avm `5a2f8c60` `VICTORY_PIPELINE_LOW_MEMORY` (16 GB PCs).
- [ ] leonida c6c9da9f rest: the fused AVX2 compare-and-copy in
      `merge_under_writes`, dynamic slots on capped heaps, pooled uploads.
- [ ] Belobug `over_cap`/`TEXTURE_MAX`, `release_tables`, by_address fix.
- [ ] Copy-queue texture uploads + show-after (avm; device-loss fix too).

### 4. No shader/pipeline building while playing
- [ ] 6.7: ~2800 host pipelines made during the run (stand-ins meanwhile),
      shader misses 0. Compare our `prepare` (recipes) with avm's pipeline
      book + `warm --used`; log how many pipelines were made in game that
      prepare had not made.

### 5. Forks to check each round
- https://portwatch-vi.xyz/ (chunks 3-5 not read yet)
- avatarmanz/VIctory-7900XTX (main + claude/* branches), Belobug a-publier,
  DexNom/VIctory, fxceless/victory-fork, stellarr-studio/kelvaroq
- Checked for 6.11: lepotane/VIctory-leonida (19.4 fps on an RTX 4060 Ti
  16 GB; memory settings sized for 16 GB cards), bolado-dev/mandy-port,
  1R0nim/iR0-fork and RandomSkelly/Series0nLinux (nothing for us).
