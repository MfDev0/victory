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
     Catmull-Rom filter and sharpens it (`VICTORY_SHARPEN`, percent, 35; 5.8:
     RCAS, 80);
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
   4.9, this fork's engine again, with AvatarManz's fixes copied in
   (avatarmanz/VIctory-7900XTX, its fixes only, not its engine):
   - **no more waiting for the card every frame** for memory the game
     changed under the card's writes: each frame of the 2026-11-25 report
     stood 400 ms for the card while only 3 pages were checked. The changed
     bytes are now put in beside the card's writes (`VICTORY_MERGE_RUNS`,
     `VICTORY_MERGE_UNWATCHED_MB`), so the CPU and the card work together;
   - **the card is kept awake** between the game's submissions
     (`VICTORY_CARD_AWAKE=1`): it stood idle 180 ms a frame and lowered its
     clocks;
   - **textures show on a full card**: the 8 GB card was full for the
     whole run and 17,000 textures waited for room, drawn black or grey.
     Now a new texture gets memory for its small levels only (blurred, with
     its colours) until there is room (`VICTORY_SQUEEZE_TILES`, 0: off);
   - **textures stop popping to black**: a slot that gets a sharper or moved
     copy of its texture keeps showing the old one until the new one is up
     (`VICTORY_KEEP_SHOWN=1`);
   - **shader fixes**: exact values from packed buffer formats (wrong
     materials), cube-map reflections at the right sharpness, unordered
     views of vertex programs declared;
   - render targets not set again when they did not change
     (`VICTORY_BIND_CACHE`), ranges of changed memory joined before the
     texture lookup;
   - the report lists which programs the dropped draws belong to.
   5.0, every shader the 2026-11-25 16:10 report named fixed in the
   translator:
   - **f133e35efed2a48a translates**: a select on a constant lane mask and
     a lane write are now taken when each 32-lane host wave runs on its own
     (each thread knows its console lane, so it is exact). The same was all
     that kept other 64-lane compute programs asking for 64-lane waves, which
     the RTX 4060 refuses (6 pipelines in every run);
   - **7f05ddab771799a9 translates**: the loop mask of lanes still looping
     (`s_andn2 rest, rest, exec`) is read as the ballot of the lanes in its
     block (`VICTORY_MASK_ELSEWHERE=0`: refused as before);
   - **ceb5eb68cd99cb07 translates**: a buffer's record count is read from
     the host view (0 for a view that is not there, which the program tests
     to skip itself: the "buffer view that is not there" warning);
   - a texture the game fills itself after its upload is now told as
     information, not a warning (it was already uploaded again);
   - **recommended settings by default**: the first 5.0 run puts every switch
     back to the recommended one (whatever a lost card or an earlier version
     switched off); **D** does it again any time;
   - the refused pipelines' console shaders are in the report, for the next
     round.
   5.1, the 6 pipelines the RTX 4060 refused in every run (they asked for
   64-lane waves, which NVIDIA cards do not have):
   - **64-lane programs split in two 32-lane halves even around shared
     memory**: the way the open VIctory pull requests run them (#1890 "split
     waves", #233), taken further where those still refuse: the halves meet
     wherever every lane runs, and are not made to meet inside branches
     only some lanes take (a meeting there would freeze the card). Close
     results instead of a pipeline that is never made and dispatches that
     are dropped (`VICTORY_WAVE_FORCE_SPLIT=0`: refused as before);
   - in that last form also: wave-wide branches, lane permutes, 16-bit wave
     sums and lane masks are taken within each half.
   5.2, the fixes of codec_d's cool_fox (the build said to reach 25 fps on
   NVIDIA cards), taken into this fork (not its engine):
   - **2da4beb386509b77 translates** (transmission prefilter): the lanes an
     access really runs for are known when join edges are pruned; its buffer
     stayed empty, so eyes, glass and alpha came out as black squares;
   - **faces no longer deform**: 16-bit data of integer buffer views keeps
     its bits (the face network's output was noise), for table views too;
   - **draws no longer lose their tables**: descriptor slots come back as soon
     as the card has run a submission, and a draw out of slots waits for them;
   - **one bad indirect record no longer empties the rest**
     (`VICTORY_INDIRECT_REST=1`: as before);
   - more 64-lane programs lowered for 32-lane cards (driver-compiled loops,
     two scene kernels, lane spills), and each view heap gets its own buffer
     refresh every frame.
   5.3, our own memory guard, after the one in codec_d's cool_fox (its
   block pool and residency module are not in this fork):
   - **upload buffers are reused**: texture and buffer copies took a new
     upload buffer from the driver for every copy and freed it again after
     the frame; now they go back to a pool by size and the next copy takes
     one (`VICTORY_UPLOAD_POOL=0`: as before);
   - **three memory levels** (normal, getting short, nearly out) from the
     memory Windows has left to commit, the free memory and the card's
     budget: idle copies are let go sooner as memory runs short, and when
     nearly out the texture pass takes half (`VICTORY_PRESSURE=0`: off);
   - **after a refused allocation**: what is kept for reuse is let go and the
     allocation asked once more, and new texture staging waits half a second
     instead of a burst of failing calls (a stutter);
   - the game log says each report which level it is at, and how many upload
     buffers were reused.
   5.4: report zips go to a folder of their own, `Downloads\VIctory-reports`
   (no more Desktop); **S** changes the folder, **E** opens it.
   5.5, black roads, buildings and people (the 2026-11-25 and 11-26 reports:
   17,000 to 27,000 textures waiting for room on 6 and 8 GB cards):
   - **cards under 8 GB of budget run as Series S once by themselves**
     (render targets of 1280x720 take far less of the card); **C** goes
     back to Series X, and the choice then stays yours;
   - the room kept for render targets is 1,536 MB on Series S and never more
     than 35 % of the card (it was 3,072 MB on every card);
   - a texture that finds no room gets its small levels (blurred, not black)
     up to the card's budget, not only up to 97 % of it;
   - the memory guard no longer halves the texture pass when nearly out;
   - the summary says how many textures still waited at the end.
   5.6, the cause of the black textures found (the 2026-11-26 report:
   1,448 textures made "level by level", 2.9 GB, and **0 MB ever given to
   them**):
   - the step that gives such a texture its memory ran *before* the check
     that reads the texture (the checks run in batches on several threads):
     it always saw no level ready, so these textures stayed black for the
     whole run. It now runs right after the checks;
   - **texture quality on 6 and 8 GB cards**: the largest level of each big
     texture (three quarters of its memory) is left out, as PC games'
     "texture quality: medium" does (`VICTORY_TEXTURE_TOP_DROP=0` to 3; one
     more while the card is full). Every big texture is made level by level
     on such cards, so it only takes memory for the levels it shows;
   - the memory for a texture's levels is given in one call instead of one
     per level;
   - small levels are given up to an eighth past the card's budget (blurred
     instead of black);
   - the Series S switch reads the card's memory from Windows itself when no
     log tells it (5.5 never found it in the log), and runs once more.
   5.7 (the 2026-11-26 12:53 report: textures now get their memory, 108 MB
   to 1,024 levels, but the run was **still Series X** and the card stood at
   5.8 GB of a 5.2 GB budget with 22,500 textures waiting):
   - **VIctory itself picks Series S** on a card with 9 GB of its own memory
     or less when Series X is asked for (the launcher's switch never reached
     a launcher that had updated itself with U). A Series X picked with
     **C** stays Series X (`VICTORY_CONSOLE_AUTO=0`);
   - **U** now hands over to the newer VIctory Try at once, so the
     launcher's own fixes apply without closing the window;
   - texture quality now reaches small textures too: textures of 256 KB or
     more are made level by level on 6 and 8 GB cards (the report's textures
     were 411 KB on average, nearly all under the old 1 MB line), and their
     largest level is left out from 256 KB up;
   - a new log line says where the card's memory goes (render targets,
     textures, tiles, staging buffers) and which console ran.
   5.8 (a sharper image, not a blurrier one):
   - the scaled-up frame is sharpened with **FSR 1's RCAS** (contrast-adaptive:
     edges and texture detail get their contrast back, flat areas and noise
     are left alone), 80 % by default (`VICTORY_SHARPEN`, percent); the old
     sharpening was mostly undone by its own clamp;
   - **16x anisotropic filtering** on every textured sampler that filters
     between levels (roads and walls seen at an angle stay sharp;
     `VICTORY_ANISO=1` to 16), and a **negative level bias** for the render
     scale (-0.42 at 75 %, as DLSS and FSR ask of games; `VICTORY_LOD_BIAS`,
     in hundredths);
   - **adaptive texture quality**: the largest levels are left out only while
     the card is past 70 % of its budget, not always; textures uploaded
     blurred get their full detail back once memory frees up (256 a frame;
     `VICTORY_TEXTURE_TOP_DROP` still fixes the count);
   - **placeholders instead of black**: a texture not on the card yet reads
     a mid grey (colour), a flat normal (normal maps) or white (occlusion and
     masks) instead of zeros, so surfaces look plain rather than black while
     they load (`VICTORY_PLACEHOLDERS=0`: off);
   - a log line counts levels left out, textures made sharp again and
     placeholders shown.
   5.9 (the 2026-11-26 13:36 report: Series S at last, the card under its
   budget at 3.7-4.1 of 5.2 GB, yet 14,000 textures still waiting: this
   engine prepares them on the CPU at about 11 MB/s, 4.3 s a frame, with the
   card 10-21 % busy):
   - **N: the engine.** By default VIctory Try now builds and runs
     **soldsawn/victory-nvidia-8gb** (soldsawn's build of
     fxceless/victory-fork, the community's main line), measured on an
     RTX 4060 Laptop 8 GB at 8-10 fps with clean textures. It is kept in its
     own folders (`victory-8gb`, `target-8gb`, `cache-8gb`), so this fork's
     build stays and N switches back at once; this fork's source is still
     fetched, so VIctory Try keeps updating itself;
   - it runs with that build's own launcher settings
     (`VICTORY_TEXTURE_DROP_MIPS=2`, `VICTORY_RESOLUTION=1920x1080`, the
     texture upload thread `VICTORY_HEAP_WORKER=1` and the rest), none of
     this fork's, plus Series S and halved card budgets on cards under 7 GB.
     Settings made before VIctory Try started are kept;
   - if it cannot be downloaded or built, this fork's engine runs instead;
   - the `8 GB engine builds` workflow checks every 6 hours that its newest
     main still compiles on Windows.
   6.0 (the 2026-11-26 14:16 report on this fork's engine: the world at 1.0-1.7 s
   a frame, not 5 s; textures are fast now, but the submitting thread spends
   0.5-1.1 s a frame on the heap passes, 160-230 ms syncing descriptor
   records, 100-150 MB of CPU compares, with the card 40-74 % idle and the
   PC under 1 GB of free memory):
   - **this fork's engine is the default again** (5.9's 8 GB engine made no
     difference); N still switches to it (`engine` = `8gb`);
   - **all of the CPU**: the threads that copy back, refresh and compare are
     all of the CPU's threads but two, at most 16 (`VICTORY_WIDE_THREADS`;
     they were a quarter, at most 6), and run above normal priority, as does
     the copy-back thread (`VICTORY_WIDE_PRIORITY=0`: normal); big textures
     are detiled on half the CPU's threads (`VICTORY_DETILE_THREADS`; 4
     before);
   - **descriptor sync on every thread**: the records the title copies into
     its heaps as bytes are looked at page by page on those threads
     (`VICTORY_HEAP_SYNC_WIDE=0`: one thread), and without a lock per record
     (one atomic per slot; fxceless #1043 found the same lock churn);
   - **the whole machine**: the process runs above normal priority, Windows
     is told not to throttle it onto efficiency cores (EcoQoS off), and it
     gets a soft minimum working set of a quarter of the PC's memory, at
     most 4 GB (`VICTORY_WHOLE_MACHINE=0`: none of it);
   - **the card first**: the graphics and copy queues ask for high priority
     (`VICTORY_QUEUE_HIGH=0`: normal); the device is made on the adapter
     Windows ranks fastest (`EnumAdapterByGpuPreference`; `VICTORY_ADAPTER=first`:
     the first one), every adapter is named in the log, and victory.exe is
     set to "High performance" in Windows' graphics settings;
   - **the card looks for shader writes** in title buffers of 4 MB and more
     (16 MB before on 6-12 GB cards; `VICTORY_CARD_BUFFERS_MIN_MB`);
   - **the CPU's own graphics as a helper (I)**: on a PC whose CPU has
     graphics of its own, the browsers, Discord, Spotify and Steam's web
     views that are running are set to "Power saving" (that chip) if they
     had no setting, so they leave the NVIDIA card's memory to the game after
     their next start; I switches it off and removes those settings again;
   - **power**: the High performance plan (and Windows 11's Best performance
     mode) while the game runs, the old one back afterwards
     (`VICTORY_TRY_POWER=0`: left alone);
   - **a build for this PC**: `-C target-cpu=native` and thin LTO
     (fxceless #1117); if that fails, the plain build
     (`VICTORY_TRY_PLAIN_BUILD=1`: plain only). The first build after the
     update rebuilds everything once.
   6.1 (6.0 ran: much better than 5.4, but people looked low-detail at first
   and buildings came in late; and U had never moved a window past 5.4):
   - **U really updates**: the newer version takes over in the same window
     with an `UPDATING` line, and starts from the settings made before
     VIctory Try started (it took the old window's run settings for the
     user's own and kept their old values); after U with nothing newer it
     says so;
   - **the version everywhere**: window titles, the first line of the game
     log (`VICTORY_TRY_VERSION`, printed by victory.exe), the report name
     (`VIctory-report-<version>-<date>.zip`) and `try-version.txt` in it;
   - **loading screens pre-load the world** (the console's way): a frame
     after one of fewer than 400 draws (`VICTORY_LOADING_DRAWS`; 0: off)
     gives the pass over textures picked by index up to 1.2 s and 768 MB
     (384 MB when the PC is short of memory);
   - **full detail until 85 %** of the card's budget
     (`VICTORY_TEXTURE_SHARP_PERCENT`; 70 before, and a 6 GB card sat at
     73 %), and 1,024 blurred textures a pass made sharp again (256 before);
   - **DirectStorage workers above normal priority** (the title's streamer
     waits for them; `VICTORY_WIDE_PRIORITY=0`: normal).
   6.2 (6.0 and 6.1: the picture right, but it takes long to come in):
   - **the picture first, the detail after**: while 1,500 or more textures
     picked by index are due (until under 300), a texture made level by
     level is detiled and uploaded without its largest levels of 512 KiB or
     more (`VICTORY_TEXTURE_RUSH`, 2 unless set, 0: off; never past the
     fourth level from the smallest): about a sixteenth of the bytes and
     CPU time, so the whole scene is there, soft, in a fraction of the
     time. When the backlog has drained, those textures are uploaded whole
     again (`sharpen`, 1,024 a pass) and textures that draws read through
     tables are detiled whole at their next use.
   6.3 (the 6.2 report, 154745):
   - **loading screens really pre-load**: the pass that runs behind a
     loading screen is the early one (12 ms and 64 MB a frame); the 6.1
     loading budget was only in the world's pass. Now a loading screen gives
     the early pass `VICTORY_LOADING_MS` (200) and 768 MB a frame.
   - **a loading screen is 30 frames of few draws in a row**
     (`VICTORY_LOADING_FRAMES`); one such frame turned the 1.2 s budget on
     in the world (passes of 750 ms to 2 s at 1-2 fps).
   - no texture rush behind a loading screen; textures it made soft are
     uploaded again once it is over (`unrush`), not only under the sharp line.
   - the pipeline library (770 MB) is saved at most every
     `VICTORY_LIBRARY_SAVE_S` (180) in the world when the PC is short of
     memory.
   6.4:
   - **the world's curtain**: for up to `VICTORY_JOIN_HOLD_MS` (10,000; 0:
     none) after a loading screen went, while 300 or more textures picked
     by index are due, the world's pass gets 1.5 s and 768 MB (384 MB short
     of memory); a line says when the curtain is down.
   - **priority** (`VICTORY_TEXTURE_PRIORITY=0`: off): the slots looked at
     again first, then the queue's order (newest and oldest write in turns)
     with three textures of up to 256 KiB to one larger.
   - **staging pool**: texture staging buffers go back to a pool by size
     class (`VICTORY_STAGING_POOL_MB`; 128, 96 short of memory, 48 nearly
     out) and are zeroed when taken again.
   - **library read-ahead** (`VICTORY_LIBRARY_PREFETCH=0`: off): the mapped
     pipeline library is prefetched at the start when the PC has its size
     and 4 GB more free.
   - Resizable BAR uploads on their own config switch (`rebar`), off again
     after a lost card; ambient light 0.45 (A: normal / low / high / off).
   6.5:
   - the world's texture pass takes at most `VICTORY_TEXTURE_FRAME_SHARE`
     (35) percent of the frame before, never under 60 ms (not behind a
     loading screen or the curtain).
   - adaptive texture quality with a band: short from the sharp line,
     sharp again 5 points under it.
   - a line every 100 passes: where the texture pass's time goes.
   6.6:
   - write-watch askings run with the journal let go; only askings of the
     same pages wait for each other (`VICTORY_WATCH_UNLOCKED=0`: as before).
   - the title's CPU buffer copies that read what a flight's shaders may
     write take the results one flight late (`VICTORY_LATE_COPIES`, 1; 2
     also copies that write there; 0 settles as before).
   - `VICTORY_CARD_FETCH_INFLIGHT=1` from Try with the relay (off with the
     3.8 additions after a lost card).
   - the ambient light follows the brightness of the lit scene the probe
     resolve reads, measured on the card (`VICTORY_AMBIENT_FOLLOW=0`: off;
     `_MIN` 0.35, `_STEEP` 0.35).
   - the world's texture pass: at most `VICTORY_TEXTURE_PASS_MS` (120) and
     a share of the eased frame time.
   6.7:
   - **saved game** before every launch: Enter continues the auto-save, N
     starts a new game from the very beginning (the title's `saves` and
     `storage` folders under `%LOCALAPPDATA%\victory\data\7952987A` are
     renamed `*.bak-<time>`, never deleted). J: new game, bring the last
     save back, ask before launch on/off. `VICTORY_TRY_SAVE=continue|new`
     skips the question.
   - **temporal upscale** (T; `VICTORY_UPSCALE_TEMPORAL=0`: the plain
     stretch): the FidelityFX upscale dispatch is answered by a resolve that
     takes the jitter off, moves its history by the title's motion vectors
     (Catmull-Rom), clips it to the 3x3 neighbourhood in YCoCg and blends a
     tenth of the new frame (`VICTORY_UPSCALE_FEEDBACK`, 10;
     `VICTORY_UPSCALE_SHARPEN`, 20). Dithered hair and jittered soft
     shadows are averaged as on the console.
   - `VICTORY_LATE_WAIT_LISTS=384` from Try (256 again after the dynamic
     descriptor slots ran out, in the run and the next ones).
   - the in-flight card fetch on a config switch of its own (`inflight`).
   6.8 ("the breakthrough"):
   - **fxceless/victory-fork merged whole** (`e4245f3c`): the heap pass on
     a worker thread (`VICTORY_HEAP_WORKER=1`, with the relay's late
     waits), texture checks beside the preparing
     (`VICTORY_PARALLEL_REFRESH=1`), settles that make room at once
     (`VICTORY_SETTLE_ROOM=1`), a per-table lookup cache
     (`VICTORY_LOOKUP_CACHE=1`), small textures copied back for the water
     (`VICTORY_TEXTURE_BACK=1`), 512 MB of buffer windows on the card
     (`VICTORY_CARD_WINDOWS_MB=512`), half views (mouths), page-write
     tracking (black faces), the card-hash on several threads. All from Try
     on **Y** (off by itself after a lost card).
   - `monitorx` / `mwaitx` are patched in place at their first trap
     (NOPs, and `pause`), with one compare-and-swap of the aligned word
     holding them (or a short jump over the rest where they cross one):
     about 125,000 exceptions a second fewer (`VICTORY_MWAIT_PATCH=0`: as
     before).
   - `VICTORY_CARD_FETCH_INFLIGHT` is off by default (it tripled the card's
     time per frame on an RTX 4050 Laptop).
   - **low preset** (**X**; on by itself under 7000 MB of card memory):
     `VICTORY_DISPLAY_MODE=1280x720` and `VICTORY_TEXTURE_BUDGET_MB=1536`.
   - the wide pool (`VICTORY_WIDE_THREADS`): the CPU's threads less two, at
     most 8 (16 before).
   - `VICTORY_CARD_BUFFERS_MIN_MB=2` (4 before).
   - a `GetModuleHandle` of a title DLL that another thread is mapping
     waits for that load instead of being refused.
   6.14 ("one frame ahead"; plan and to-do in `scripts/try/PLAN.md`):
   - `VICTORY_COPY_FETCH` and `VICTORY_TRIM_BARRIERS` on their own Try
     switches (config `copyfetch`, `trim`), no longer tied to the 3.8 set
     (`new38`) a lost card had turned off; lazy copy-back on again once.
   - `VICTORY_ASYNC_SUBMIT=1` and `VICTORY_ASYNC_PRESENT=1` (config `ahead`;
     Kelvaroq by stellarr-studio, patches 0040/0048).
   - `VICTORY_CARD_HASH_WIDE=1` (config `widehash`; fork #1014, Kelvaroq
     0041).
   - Relay fetch copies of neighbouring pieces joined
     (`VICTORY_RELAY_COPY_RUNS=0` goes back; Kelvaroq 0032).
   6.13 ("NVIDIA fixes"; plan and to-do in `scripts/try/PLAN.md`):
   - `VICTORY_WINDOWS_BAR_MB` (trustno1-coder/VIctory-NVIDIA, Indy2l):
     windows outside buffers in a GPU upload heap (Resizable BAR), own cap,
     refused past 80% of the card; Try sets it under 24 GB of RAM (20% of
     cards under 8 GB, 30% above, max 3072; MAX 35%; config `barwin`).
   - `VICTORY_TEXBACK_ASYNC` (on; Indy2l): texture read-backs written by a
     thread of their own.
   - `VICTORY_PIPELINE_WAIT_ONCE` (on) and `VICTORY_PIPELINE_EARLY_COMPUTE`
     (on) (edsuuu/VI-BR-PORT).
   - Decoration bake programs by name; texture change lists compacted at
     2^18; card compare on its own `ExecuteCommandLists` (nv d1fe0bd0).
   6.12 ("all-out"; plan and to-do in `scripts/try/PLAN.md`):
   - **MAX: all-out mode** (`VICTORY_TRY_ALL_OUT=1` too): VIctory's caps
     lifted to what the PC has but ~1% (threads-1 for copy-back, detiling
     and pipeline compiles; high priority; textures to 99% of the card, no
     texture cap; the memory guard at 1% free). New settings behind it:
     `VICTORY_PIPELINE_WORKERS`, `VICTORY_PIPELINE_HEAVY`, `VICTORY_PRIORITY`,
     `VICTORY_WORKING_SET_PERCENT`, `VICTORY_PRESSURE_RAM_MB`,
     `VICTORY_PRESSURE_COMMIT_MB`, `VICTORY_PRESSURE_CARD_PERCENT`;
     `VICTORY_TEXTURE_ROOM_PERCENT` up to 99.
   - Passes that only serve rays are neither built nor run with
     `VICTORY_SKIP_RT=1` (edsuuu/VI-BR-PORT): bvh skinning, ray resolve,
     probe ray picking, traces against unbuilt structures.
   - `scripts/try/rt-off-hashes.txt` as `VICTORY_DENY_PIPELINES` with B, and
     `VICTORY_NAN_SCRUB=1` (trustno1-coder/VIctory-NVIDIA).
   6.11 ("no disk stutter"; plan and to-do in `scripts/try/PLAN.md`):
   - The pipeline library's read-ahead leaves 1 GB free (4 GB before;
     `VICTORY_LIBRARY_PREFETCH_ROOM_MB`), and reads the front of the file
     when the whole does not fit: on a 16 GB PC it was skipped, and every
     pipeline loaded in play waited on the disk.
   - `victory prepare <game folder>` once per build (Try; config
     `shaders_all`, P resets it, `VICTORY_TRY_ALL_SHADERS=0` skips it):
     every shader of the game translated ahead, a progress line every 2,000.
   - `VICTORY_JOIN_PARTS=1` in the Y set (fxceless's speed set).
   - `WIDE_JOB` 256 KiB (lepotane/VIctory-leonida c6c9da9f).
   - Anisotropic point-mip samplers kept where `D3D12_OPTIONS19` says the
     card has them (avatarmanz 011735ff, upstream e45bacd/23fc8a5/7dfa86f),
     also through our sharper-surfaces override.
   - xtask policy: Rust sources up to 1 MiB.
   6.10 ("own engine + AMD upscaler"; plan and to-do in `scripts/try/PLAN.md`):
   - **This fork's engine is the default again and stays the base**; avm
     and 8GB are on N for comparing only. Fixes of the other forks are
     ported into it.
   - **PC FidelityFX upscaler** (Belobug cd8dfc06, ported there from
     DexNom ab34a8b2): `VICTORY_FIDELITYFX_DIR` names a folder with AMD's
     signed `amd_fidelityfx_upscaler_dx12.dll` (FidelityFX SDK 2.4.0); the
     title's FidelityFX loader is then served by it on the host GPU
     (`victory-hle/src/modules/fidelityfx.rs`, `victory-gpu/src/external.rs`).
     Here for every profile (Series S too), FSR 3.1 where FSR 4 does not
     run, `VICTORY_FIDELITYFX_PROVIDER` picks one by name, and a dispatch it
     cannot record goes to our temporal resolve. Try downloads the DLL once
     into `%LOCALAPPDATA%\VIctoryTry\fidelityfx` and checks AMD's signature.
     T: FSR / own / off.
   - Our temporal resolve: Catmull-Rom colour, sharpening against a tent
     blur (no rendered-grid blocks), `VICTORY_UPSCALE_SHARPEN` 10 by default.
   - avm's native shader reader (unpacked kind 0x0c, FSPX/ACCC layout).
   - Logs: the upscaler in use, the providers offered, jittered upscales
     every 600.
   6.9 ("AvatarManz engine"; plan and to-do in `scripts/try/PLAN.md`):
   - **The default engine is AvatarManz's** (PortWatch #1 and #2:
     Belobug ~22 fps on an RX 9070 XT, AvatarManz 16 fps / 45 ms on an
     RX 7900 XTX): `Belobug/VIctory-Belobug-AMD` branch `a-publier`, which
     is `avatarmanz/VIctory-7900XTX` c9c932c plus Belobug's flicker fix,
     VRAM-aware texture cap and FSR 4.1.2 (`VICTORY_TRY_AVM=avatarmanz`:
     avatarmanz main itself). Source in `%LOCALAPPDATA%\VIctoryTry\victory-avm`,
     build in `target-avm\diag`, cache in `cache-avm`; saves are shared.
     Built with their command (`cargo build --locked --profile diag -p
     victory --features diagnostics`), for this CPU first. If it does not
     build, this fork's engine runs. **N** goes avm -> own -> 8GB.
   - Run with their `scripts/play.ps1` settings, renderer patches 1, 2, 5,
     6, 7 on (`VICTORY_PORT_CONSTANTS`, `_DESCRIPTORS`, `_CBV`, `_GUARD`,
     `_JOURNAL`; **Y**), `VICTORY_PIPELINE_BOOK=1`.
   - With the game closed: `victory shaders` (every shader, once per build)
     and `victory warm --used` (the pipelines earlier runs drew with), so
     nothing met before compiles while playing (`logs\prepare.log`).
   - Its menu: **B** ray tracing (`VICTORY_BVH_BUILDER=1`), **X** textures
     (auto / sharp `VICTORY_TEXTURE_MAX=2048` / low 512), **A** ambient
     (0.8 / 0.5 / 1.1 / 0), **F** above 30 fps
     (`VICTORY_FRAME_INTERVAL_MS=16.667`), **Z** their -Safe set, **R**
     output size on Series X (`VICTORY_OUTPUT`).
   - After a lost card: ray tracing off, then Safe, then patches off. An
     out-of-memory end prints the page-file fix (their engine takes 30-46 GB
     of RAM + page file for now). RAM and commit limit are shown each run.
   - The report zip has `settings.txt` (every `VICTORY_` setting, RAM,
     commit limit, card memory, threads) and `prepare.log`.
   - This fork's engine: avatarmanz's output size / render scale / upscaler
     patches (`VICTORY_OUTPUT`, `VICTORY_TITLE_SCALE`, `VICTORY_FFX_SCALE`,
     `VICTORY_UPSCALER`; victory-loader `resolution.rs`).
5. When the game stops (3.2), it prints a short summary (typical and best
   fps, shaders that did not translate, pipelines the card refused, slow
   submissions) and puts everything to send in one file,
   `VIctory-report-<date>.zip`, in its own folder in Downloads,
   `Downloads\VIctory-reports` (5.4: **S** in the menu picks another folder,
   **E** opens it) (the game log, crash details, build log and
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
   - **S**: choose where report zips are saved, **E**: open that folder;
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
