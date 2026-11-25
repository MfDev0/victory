# VIctory Try

A simple way to try this fork's VIctory on your own PC. Download
**VIctoryTry.exe** from the fork's Releases page, double-click it, and type
two paths: the game files folder and the game's exe. That's all.

It doesn't sign in to anything and doesn't send anything anywhere. It only
downloads things anonymously:
- this fork's source code, and upstream VIctory's new commits;
- the first time only, the tools building needs: Git (a portable copy if you
  have none), Rust and Microsoft's C++ Build Tools (a few GB; Windows asks
  for permission once).

## Upstream's new fixes, added by themselves

Each time it updates, VIctory Try also fetches upstream VIctory
(Marvisak/VIctory). [`upstream.txt`](upstream.txt) names the last upstream
commit this fork already has. Every later upstream commit is applied on top
of the fork, oldest first:
- a fix the fork already has applies as an empty change and is skipped;
- a fix that clashes with the fork's changes is skipped;
- if the build fails with the new fixes added, the fork is built alone, so
  you always get a working build.

The window lists what was added and what was skipped.

## Keeping itself up to date

From 1.3 on, VIctory Try checks the branch's `scripts/try/try.ps1` each time
it updates. If that one is newer than the one inside your VIctoryTry.exe, it
runs the newer one, so fixes to VIctory Try itself need no new download.

## Safety checks

- The game exe can't be VIctoryTry.exe, victory.exe or anything inside
  `%LOCALAPPDATA%\VIctoryTry`. Choosing VIctoryTry.exe as the "game" made
  VIctory run VIctory Try, which started itself again in a loop. A saved path
  like that is rejected and you are asked again.
- A VIctory Try started from inside a game run stops at once.
- Only one VIctory Try runs at a time, because two would build in the same
  folders.
- When a build fails, the window shows the first error.

## What it does

1. Asks for the game files folder (the one with the `.rpf` archives) and the
   game exe, which may be in another folder, and remembers them. VIctory uses
   the folder holding the game's archives as the console's `G:` drive. If the
   folder holds several exe files, it lists them so you can pick one by number.
2. Installs Rust and the C++ Build Tools if they are missing.
3. Downloads the newest source, adds upstream's new fixes, and builds `victory.exe`. The first build takes
   a while; after that it only rebuilds when the source changed.
4. Runs `victory.exe run <game exe> --execute --console series-x` and shows
   the output. From 3.0 the fork stands on fxceless/victory-gta6, which
   renders the world on NVIDIA as a Series X (the Series S profile loses the
   device before the first frame there). Defaults: shader loop limit 256,
   `VICTORY_RELAY=2`, the two programs known to hang the card skipped, and
   a cache folder of its own.
   Each run keeps a recipe of every pipeline the game asks for (3.5). After
   the game closes, and before the first run of a new build, VIctory Try
   runs `victory.exe prepare`: with the game closed it compiles every one of
   them into the pipeline library, on all cores but two (output in
   `logs\prepare.log`). The next launch has them all from its start, so
   one run that reaches a place is enough: no restarting again and again
   while the world fills in a few pipelines a minute.
   From 3.6 the compute shaders that ask for 64-lane waves, which 32-lane
   NVIDIA cards refuse ("pipelines the card refused"), are translated in a
   slower form that the card accepts (`VICTORY_WAVE_WIDE=0` turns it off).
   3.7 adds fxceless/victory-gta6's newest work: textures a draw needs are
   put on the card before it, faster defaults (relay, copy-back on all
   threads), packed textures stay queued, skin and hair colours checked
   again after upload, and the fx-prs fixes (`VICTORY_RELAY=0`,
   `VICTORY_WIDE_BACK=0` turn the new defaults off).
   3.8 adds, each with its own switch to turn it off:
   - a draw whose pipeline is still being made is drawn with a ready one of
     the same shaders instead of left out (`VICTORY_STAND_IN=0`);
   - texture checks (read, compare, detile) on all cores at once
     (`VICTORY_WIDE_TEXTURES=0`);
   - upload memory in the card's own memory when Resizable BAR is on
     (`VICTORY_GPU_UPLOAD=0`);
   - buffers the card's memory has no room for compared on the card, their
     shaders' side in the PC's memory, up to 768 MB (`VICTORY_PC_CARD_MB=0`);
   - empty views no longer counted as refused, and the first calls of the two
     unknown pipeline methods write their pieces to the log for the next fix
     (`VICTORY_RAW_TYPED=1` tries a fix for raw views off 16 bytes).
   3.9 adds, each with its own switch to turn it off:
   - texture checks no longer wait while the game streams memory in and out
     (that wait was nearly all of the 1-2 s "textures" spikes;
     `VICTORY_TEXTURE_NO_WAIT=0`);
   - lazy copy-back: the game hears the card is done at once, and a game
     thread that touches what the card wrote waits only for that
     (`VICTORY_LAZY_BACK=0`; off by itself after a crash on a memory access);
   - what the card wrote is fetched on its copy engine, not behind the
     drawing work (`VICTORY_COPY_FETCH=0`);
   - only bytes that really changed go to the card's copies
     (`VICTORY_SAME_SKIP=0`);
   - pipelines that the most draws wait for are compiled first
     (`VICTORY_PIPELINE_MOST_ASKED=0`);
   - DirectStorage reads up to 128 MB at once instead of 32
     (`VICTORY_DSTORAGE_STAGING_MB`).
   4.0 adds, each with its own switch to turn it off:
   - copy back only what the game reads (like Dolphin skipping EFB copies):
     buffers nothing on the CPU has touched for 64 batches stay on the card
     (`VICTORY_READ_BACK_ONLY=0`);
   - late results (like Xenia's fast readback): the game's uploads no longer
     wait for the card's last results to come back; they use the ones before
     (`VICTORY_LATE_RESULTS=0`);
   - a second kind of stand-in pipeline (Dolphin's ubershader idea): an
     object whose pipeline is still compiling borrows one with the same
     shaders that only writes depth differently, instead of vanishing
     (`VICTORY_STAND_IN_LOOSE=0`);
   - a shared pipeline list: pipelines other players met start compiling
     while the game loads (`VICTORY_SHARED_PIPELINES=0`); your own list goes
     into the report zip (`early-pipelines.txt`), so it can be added;
   - large 2 MB memory pages for the game's CPU memory, if Windows allows it
     ("Lock pages in memory"; `VICTORY_LARGE_PAGES=0`);
   - barrier trimming: GPU waits between two reads are dropped
     (`VICTORY_TRIM_BARRIERS=0`; off by itself after the card is lost).
   Occlusion queries already answer "visible" at once (nothing hidden by
   them).
   4.1 fixes the build: up to 4.0 a failed build still counted as built
   (cargo's lines became the build step's answer), so the old victory.exe
   kept running and none of 3.9 or 4.0 reached the game. Upstream's new
   commits are no longer added by default (one of them, upstream #1180,
   does not build with this fork; `VICTORY_TRY_UPSTREAM=1` adds them, and
   falls back to the fork alone when they do not build). It also brings
   fxceless's newest fixes (the ones ztrx is made from): a write through one
   view of a memory page is seen at its other views, descriptors the game
   copies as bytes are mirrored (the first room renders right), a much
   cheaper descriptor sync, and a shader cache that survives updates (no
   re-translating every shader after each update, so less stutter at the
   intro).
   Also new in 4.1:
   - the screen the game is told it has is 1080p (a Series X on a 1080p
     TV); at 4K it rendered 2560x1440 inside and upscaled to 3840x2160 every
     frame. **R** switches between 1080p and 4K;
   - **Z** switches plain mode: VICTORY's own defaults for every speed and
     safety switch, exactly as ztrx's launcher runs the game, to compare
     the picture with Try's settings;
   - the intro stutter: one pipeline worker is kept for pipelines the game
     is drawing with right now, and pipelines only started early no longer
     run above the game's own threads (`VICTORY_PIPELINE_KEEP_WORKER=0`,
     `VICTORY_PIPELINE_EARLY_BOOST=1` restore the old way).
   4.2 brings seven frame-rate ideas taken from other places:
   - **render scale** (as DLSS/FSR mods do): the game is told a screen 75%
     the size (`VICTORY_RENDER_SCALE`, 50-100) and renders every pass that
     small; our present scales each frame up to the window with a clamped
     Catmull-Rom filter and sharpens it (`VICTORY_SHARPEN`, percent, 35);
   - **no ray tracing** (as "RT off" mods): the console's ray structures
     are not built here, so the rays walked empty memory, cost the card time
     and gave black shadows and overbright light; with `VICTORY_SKIP_RT=1`
     every ray misses at once. **B** switches both of these (on by default);
   - **frames in between** (as VR's spacewarp): `VICTORY_FRAMEGEN=1` (**M**)
     finds how each 16x16 block moved between the game's last two frames
     and shows the last one moved on along that motion every
     `VICTORY_FRAMEGEN_MS` (33) until the next arrives. The fps line still
     counts only the game's frames;
   - **copy back only what is read** (as CUDA's unified memory): buffers in
     the PC's memory that nothing on the CPU reads are no longer compared
     after every submission (350-520 MB a frame in the last reports); a
     read while the results are held has them compared first
     (`VICTORY_READ_BACK_ONLY_PC=0` restores; `VICTORY_QUIET_LANDINGS`,
     `VICTORY_PROBE_EVERY`);
   - **shared small buffers** (as one-memory machines): `VICTORY_INPLACE=3`
     (**H**) makes buffers up to `VICTORY_INPLACE_SMALL_MB` (8) the game's
     own memory, shared with the card, so they are neither copied in nor
     compared; the big ones stay on the card. Off by default (riskier);
   - **reuse**: a constant buffer many draws of a list name is copied to
     upload memory once, not once per draw (`VICTORY_SHARE_CONSTANTS=0`);
   - **textures detiled on 4 threads** (`VICTORY_DETILE_THREADS`, 1 for the
     old way).
   4.3: the game's output (its rendering lines) opens in a window of its
   own, "VIctory - game output"; the VIctory Try window keeps the menu,
   updates, versions and settings. It closes by itself when the game stops
   and the summary appears in the menu window (`VICTORY_TRY_ONE_WINDOW=1`
   keeps everything in one window, as before).
   4.4, the picture fixes (each on by default):
   - **ambient light** (A; from Belobug/VIctory-Belobug-AMD): the game's
     bounced light comes from ray-traced probes that do not run here, so
     everything the sun missed was black. A soft constant light replaces
     it, shaded in corners from the depth (`VICTORY_AMBIENT=<level>`, 0.6;
     `off`; `VICTORY_AMBIENT_TINT=r,g,b`, `VICTORY_AMBIENT_SHADE`);
   - **textures level by level** (L; fxceless's `VICTORY_LEVELS_ON_DEMAND`):
     a texture takes the card's memory only for the sizes the game holds.
     On an 8 GB card about 4.9 GB of textures filled it and some 16,000
     more waited for good: doors, props and people that streamed in later
     stayed black or missing;
   - **no stop when a pool is full** (mangrove's `VICTORY_POOL_NO_ASSERT`,
     VIctory #698): the game stopped about 8 minutes in;
   - pixel shaders read the other pixels of their 2x2 group through quad
     reads (Belobug's fix for lines on car paint, glass and skin). The
     first run after this update translates the shaders again.
   4.5, walls, ceilings and loading (from the 4.3 reports; each on by default):
   - **shader clean-up off** (O; `VICTORY_SHADER_OPT=0`): the clean-up
     passes left the prison cell's walls, roof and floor out (sky where
     they should be) on an RTX 4080 and an RX 7800 XT; off, they are back
     (Xeltax/VIcky, same-save A/B);
   - **shadow fix** (W; from Xeltax/VIcky 05ee4e9, fxceless/victory-fork
     #62): on 32-lane NVIDIA cards the 14 cascade-shadow programs are wrong
     in one 64-lane form (missing ceiling, dark room); they are kept out of
     it (`VICTORY_WAVE_DEEP_SKIPS_NOT=<hash>,...` sets the list, `none`
     empties it; `VICTORY_WAVE_DEEP_SKIPS_ONLY` the other way round);
   - one more compute program translates on 32-lane cards (`f133e35efed2a48a`,
     fxceless/victory-fork #190 by gdawid94);
   - streaming: the card's page-mapping copies map runs of pages as one
     view instead of one 64 KiB page at a time under the lock texture reads
     wait on (Felix2180, fxceless/victory-fork #823): up to 2 s a frame of
     waiting while the world streams in;
   - the summary no longer calls the ~370 draws a frame skipped at the end
     "waiting for pipelines": with none being made they are compute work
     (mostly the ray-tracing builder, which is off), not missing objects.
   The first run after this update translates the shaders again.
   4.6, Series S and the command list:
   - **Series S shows the game** (C; fxceless/victory-fork #188 by
     sagouindupm and Filip722): in Series S mode the game presents two
     planes, its picture (1920x1080) and its interface over it (3840x2160,
     see-through where empty), and calls the console's FidelityFX upscaler
     to make the picture from a 1280x720 render. Only the first plane was
     shown and the upscaler's output stayed black: a black window after
     loading. The interface is now drawn over the picture, and the upscale
     dispatch becomes a plain stretch of the 720p render. In Series S, Try
     leaves out the 75% render size and the frames in between (the game
     already renders smaller); ray tracing stays skipped with B;
   - **the menu lists every command**: its key, what it is, whether it is
     on now and what it does. Switches flip at once and show the list
     again; Enter runs the game. V shows the version.
   4.7, speed (K switches both):
   - **the game's buffer copies run on the card** (pablogonz12/Scarlight #86,
     `VICTORY_GPU_COPIES`): a copy between two buffers that already have a
     copy on the card is one copy command in the card's list, in order with
     the game's work, and the result goes back to the game like anything
     else the card writes. On the CPU, each such copy that touched memory
     the card was still writing made us wait for the card to finish all it
     had been sent: about 157 ms of every 485 ms frame in the 2026-11-25
     report (the card was idle almost half the frame). The frame-rate line
     now says how many copies ran on the card;
   - **the card's checks are joined** (`VICTORY_CARD_REHASH_RUNS=1`): after
     each upload the card re-checks the pieces it received, about 1,900
     tiny one-after-the-other passes a frame; now one pass per run of
     pieces. The same pieces are checked either way.
5. When the game stops (3.2), it prints a short summary (typical and best
   fps, shaders that did not translate, pipelines the card refused, slow
   submissions) and puts everything to send in one file on the Desktop,
   `VIctory-report-<date>.zip` (the game log, crash details, build log and
   the failed shaders). If the graphics card was lost during the run, the
   next run is safer by itself, one step a time: back from F mode to the
   relay, or the 3.8 additions off, or the 64-lane compute shaders left out again (3.6), or copy-back
   on one thread (3.5), or the shader loop limit from 256 to 128. Nothing
   changes after a good run.
6. Then you choose what's next:
   - **Enter**: run again;
   - **U**: update to the newest version and run;
   - **G**: change the game paths;
   - **C**: switch between Series S and Series X (remembered);
   - **P**: prepare all pipelines again (done by itself after each update and run);
   - **B**: switch the fps boost (75% render scale, no ray tracing; on by
     default), **M**: frames in between, **H**: shared small buffers (4.2);
   - **F**: switch to the faster speed mode (late waits and card buffers:
     4-5 fps on a 12 GB card, riskier on 8 GB) and back (remembered);
   - **Q**: quit.

## Requirements

VIctory's own requirements apply:
- Windows 10 or 11 (x64);
- a CPU with AVX2;
- a D3D12 GPU with Shader Model 6.6 and mesh shaders;
- your own copy of the game.

Keep the game outside the `%LOCALAPPDATA%\VIctoryTry` folder.

## Where things go

Everything lives in `%LOCALAPPDATA%\VIctoryTry`:
- `victory\`: the source;
- `target\`: the build;
- `logs\`: the build output and every game run;
- `config.json`: your two paths.

VIctory writes crash details to `%LOCALAPPDATA%\victory\data\crashes.log`. To
remove everything, delete those two folders.

## Windows warnings

The exe isn't signed, so Windows SmartScreen may say "Windows protected your
PC". Click **More info**, then **Run anyway**. The exe is small: it holds
[`try.ps1`](try.ps1), writes it to `%LOCALAPPDATA%\VIctoryTry` and runs it
with Windows PowerShell. You can read the script, and the exe's C source is in
[`build-exe.sh`](build-exe.sh).

## Building the exe

On any OS with Zig (`pip install ziglang` works), run
`scripts/try/build-exe.sh`. The exe lands in `target/try/`. It's a release
download only: never commit it.
