# VIctory Try: plan and to-do (6.10, updated for 6.11)

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
| kelvaroq | RTX 5080 | ~10 | CPU submission-bound; unified memory layer not public yet |
| this fork | RTX 4050L | 1.6-3.3 (6.7) | preparing 84 ms, heap passes 92 ms, relay wait ~225 ms a frame |

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
- [x] CI: xtask policy failed on `host.rs` (587 KB) and `lower.rs` (545 KB)
      over the 512 KiB limit; Rust sources may be 1 MiB now.
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

### 1. Read the 6.11 report (needs the user's zip)
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
