# VIctory Try: plan and to-do (from 6.9)

Goal: the whole picture rendered, **20+ fps** on the reporter's PC: RTX 4050
Laptop (5921 MB, budget about 5153 MB), 12 threads, about 16 GB of RAM (8 GB
"shared" in the 6.7 log), Series S profile, game 1.0.2.1.

How the loop works: each version below ships, the user runs it, sends the
report zip (now with `settings.txt` and `prepare.log` inside), we read it, fix,
ship the next one. Every fork's progress is checked each round (PortWatch,
then the forks below), and what helps is taken with its author named.

## Where everyone stands (2026-11-26, PortWatch + their READMEs)

| Fork | Card | fps | Notes |
|---|---|---|---|
| Belobug/VIctory-Belobug-AMD (`a-publier`) | RX 9070 XT | ~22 | #1. **= avatarmanz c9c932c + 3 commits** (flicker fix, VRAM-aware texture cap, FSR 4.1.2) |
| avatarmanz/VIctory-7900XTX | RX 7900 XTX | 16-22 (45 ms Series S) | #2. Renderer patches 1-7, THIN lists, copy-queue textures, warm/shaders ahead. 30-46 GB RAM, 10.5 GB VRAM |
| DexNom/VIctory | RX 7900 XTX | 3-4 | Best picture: ray tracing, cube gradients, sparse textures. Taken by avm already |
| stellarr-studio/kelvaroq | RTX 5080 | ~10 | Unified memory layer "in testing", not public yet: watch it |
| fxceless/victory-fork | RTX 4070 | 8-9 | Merged whole in 6.8 (e4245f3); avm started from it (a26859f) and is 556 commits ahead |
| this fork (own engine) | RTX 4050L | 1.6-3.3 (6.7) | Preparing 84 ms, heap passes 92 ms, relay wait ~225 ms a frame |

Conclusion: the gap between our own engine and avm's is 556 commits and
~87,000 lines on another base. Porting it piece by piece into ours, with no
compiler here and CI as the only check, would take weeks and break things.
**6.9 builds and runs avm's engine itself (Belobug's branch of it) as the
default**, the same way 5.9 ran soldsawn's 8 GB engine, and tunes it for a
6 GB card. Our engine stays one key away (N) and keeps getting the pieces
that port cleanly.

## Done in 6.9

- [x] AvatarManz engine as the default engine (N: avm -> own -> 8GB). Built
      with their command (`cargo build --locked --profile diag -p victory
      --features diagnostics`), CPU-tuned first, plain if that fails; falls
      back to our engine if it does not build.
- [x] Their `scripts/play.ps1` settings, all of them (THIN, JOIN_PARTS=8,
      UPLOAD_AHEAD, RELAY=2, CARD_STEPS=2, SHOW_AFTER, ...).
- [x] Renderer patches 1, 2, 5, 6, 7 on (`VICTORY_PORT_*`; byte-checked;
      47.3 -> 44.9 ms there). Y turns them off.
- [x] **Shaders no longer compiled while playing**: `victory shaders` (every
      shader of the game translated, once per build) and `victory warm --used`
      (every pipeline earlier runs drew with, made in separate processes)
      run with the game closed; `VICTORY_PIPELINE_BOOK=1` records them.
- [x] Picture switches: B ray tracing (lit rooms, right reflections), X
      textures (auto by free VRAM / sharp 2048 / low 512), A ambient
      (0.8 default), F above 30 fps, Z their -Safe set, R output size
      (Series X, `VICTORY_OUTPUT`).
- [x] Fallbacks after a lost card: RT off -> Safe -> patches off. Out of
      memory is detected and the page-file fix is printed.
- [x] RAM + page file checked before each run (their engine needs 30-46 GB
      of commit for now; a 16 GB PC needs a big page file).
- [x] CI: `engine-avm.yml` checks Belobug a-publier and avatarmanz main
      build on Windows with Try's exact command, every 6 hours too.
- [x] Our engine: avm's `VICTORY_OUTPUT` / render scale / upscaler patches
      (victory-loader `resolution.rs`). Note: `VICTORY_DISPLAY_MODE`, which
      6.8's low preset set, does not change the title's size (avm found the
      title uses its own mode list).
- [x] Logs: report zip has `settings.txt` (every VICTORY_ setting, RAM,
      commit limit, card MB, threads) and `prepare.log`.

## To do, in order (each needs a report from the PC to measure)

### 1. Make the avm engine run on 6 GB / 16 GB (6.9 report)
- [ ] Read the first 6.9 log: did it build, reach the world, fps, card
      memory (Belobug's HUD lines), private memory, crashes.
- [ ] If it runs out of RAM: find the switches that cut avm's memory
      (lists' allocators 9 GB, windows 7 GB, uploads 4 GB per findings.md;
      `VICTORY_MEMORY_MAP=1` lists it) and set them for 16 GB.
- [ ] If VRAM overflows (frames > 200 ms, "Critical"): texture cap low,
      `VICTORY_CARD_WINDOWS_MB`, `VICTORY_TEXTURE_BUDGET_MB`.
- [ ] Threads: THIN's 8 + JOIN_PARTS 8 on a 6-core/12-thread laptop CPU: try
      4/6 and compare the submit thread's time.

### 2. Frame rate to 20+ (RTX 4050L is ~40-50 % of a 7900 XTX)
- [ ] Series S profile + output 1664x936 (`VICTORY_OUTPUT`) if the card is
      the wall; stay at 1080p if the submit thread is.
- [ ] `VICTORY_FRAME_INTERVAL_MS` only after the world is above 30.
- [ ] Watch avm/Belobug for renderer port 8+ and take each one.
- [ ] Kelvaroq's unified memory layer when it is public (removes the copy
      loop that is two thirds of a frame on every fork).

### 3. Picture
- [ ] "Characters look like PS3": textures capped by free VRAM (Belobug's
      cap drops to 512/256 above 92/97 % of the budget). Try X=sharp, and
      Series S's 720p internal size; FSR 4 (`VICTORY_FSR_4K`) is AMD-only.
- [ ] "Pixelated light reflections": screen-space reflections at the
      internal size, without ray tracing. Try B (ray tracing) on a short run;
      DexNom's cube-gradient fix is in avm already.
- [ ] "Wrong light reflections / too dark rooms": ray tracing off has no
      indirect light; avm's ambient stand-in settles by time
      (`VICTORY_AMBIENT_SETTLE=250,3`, default there). A: ambient level.
- [ ] Trees/signs flicker far away: Belobug's prelude upload fix is in;
      `VICTORY_INDIRECT_REST=0` is avm's experiment.

### 4. Logs needed for the above
- [x] settings.txt + prepare.log in the zip.
- [ ] Belobug's HUD has VRAM by kind: ask for an F3 screenshot in the world.
- [ ] Count of pipelines made in game vs warmed (to prove no compiling).

### 5. Our own engine (kept, N)
- [ ] Port avm's light settle (ambient.rs differs too much: rewrite).
- [ ] Port the copy-queue texture upload + show-after (device loss fix).
- [ ] Port renderer port 1 (constants heap in place) if the avm engine
      cannot run on 16 GB.

## Forks to check each round
- https://portwatch-vi.xyz/
- avatarmanz/VIctory-7900XTX (main, claude/* branches)
- Belobug/VIctory-Belobug-AMD (a-publier)
- DexNom/VIctory, fxceless/victory-fork, stellarr-studio/kelvaroq
- New on PortWatch, not checked yet: 1R0nim/iR0-fork, bolado-dev/mandy-port,
  lepotane/VIctory-leonida, RandomSkelly/Series0nLinux
