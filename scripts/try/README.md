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
   6.24 ("NVIDIA DLSS"; trustno1-coder/VIctory-NVIDIA main 959dcaf, see
   `docs/dlss.md`):
   - **DL**: `VICTORY_DLSS=1`, `VICTORY_DLSS_MODE` (quality by default;
     balanced, performance, ultraperformance, dlaa, off),
     `VICTORY_STREAMLINE_DIR=%LOCALAPPDATA%\VIctoryTry\nvidia-sdk\Streamline\bin\x64`
     (downloaded once, NVIDIA-signed), GeForce RTX only; on Series X
     `VICTORY_OUTPUT=2560x1440` (R: 3840x2160) and no `VICTORY_RENDER_SCALE`.
   - **FG**: RTX 40/50 with GPU scheduling (HwSchMode 2, or unset on
     Windows 11) and `VICTORY_ASYNC_SUBMIT=1`: `VICTORY_FRAMEGEN=1`,
     `VICTORY_FRAMEGEN_X` 0 (ready; the F1 menu switches it live) or 2-4;
     `VICTORY_TEXTURE_ROOM_PERCENT` at most 86 (80 when on).
     Otherwise, with DLSS, M's `VICTORY_FRAMEGEN=1` is cleared: in the engine
     `VICTORY_FRAMEGEN=1` alone is our frames in between, and with
     `VICTORY_DLSS=1` or `VICTORY_FRAMEGEN_X` it is DLSS Frame Generation.
   - **FPS**: `VICTORY_FRAME_INTERVAL_MS` 16.667 (default), 8.333, 1 (none),
     unset (30, the game's own). `VICTORY_TITLE_FPS=<n>|off` overrides the
     game's rate.
   - **MOV** `VICTORY_SKIP_BINK=1`; **FS** `VICTORY_FULLSCREEN=1`
     (`VICTORY_FULLSCREEN_RES`); **MOUSE** `VICTORY_MOUSE_CAPTURE=1`
     (`VICTORY_MOUSE_WARP`); F1 menu only: `VICTORY_IME_ENGLISH=1`.
   - The F1 menu saves `VICTORY_MENU_DLSS`, `_FRAMEGEN`, `_FULLSCREEN`,
     `_MOUSE_CAPTURE`, `_IME_ENGLISH` to `VICTORY_MENU_FILE`
     (`%LOCALAPPDATA%\VIctoryTry\victory-menu.txt`); Try takes it in once
     per save. `VICTORY_MENU=0`: no menu.
   - Engine defaults: `VICTORY_ONE_PAD` (=0: a device per pad),
     `VICTORY_DUALSENSE` (unset auto, 0 off, 1 both), `VICTORY_RAW_VIEW_CLAMP`
     (=0 off), `VICTORY_HELPER_PRIORITY` (=0 off). `VICTORY_KEYS` and
     `VICTORY_KEYS_FILE` in every build; `VICTORY_WINDOW_MAX=WxH`.
   6.28 ("his engine, our report"): engine `nvidia` (`EngineNv`; N
   toggles own <-> nvidia, NO cycles avm/8gb). `Update-EngineNv` clones
   `$EngineNvUrl` main into `victory-nvidia`; `Build-EngineNv` checks his
   head out, `git am -3` our `scripts/try/nvidia-report-patch.txt` (read
   from refs/remotes/origin/fork by `Nv-Patch`), falls back to
   `$EngineNvBase` and then his code alone, and builds with his command into
   his `target\release`. `Run-InOwnWindow` runs `run-nvidia.ps1`, which
   calls his `scripts\play-foxhound.ps1` with options from `Nv-RunSpec`
   (`run-nvidia.json`); `Nv-SyncMenu` takes his F1 menu's choices back.
   The patch is `git format-patch d12eb1b..` of: run.rs version line
   (0f3d957), refused pipeline dumps (91a1575), root signature refusals
   (0fd37a4), checked stack walk (329e1f1), failed shader dumps, OOM log
   (6e2c856). Not in it: cf86454 (refusal reasons; HostPipeline::new's
   returns differ in his code).
   6.27 ("twelve modes"): `Quick-Menu` lists `$ModeDlss` x 2 consoles
   (1-6 Series X, 7-12 Series S; `Set-Mode` sets console + dlss only);
   F/C/X cycle fg, fps and allout in place and redraw; R resets and
   redraws; A is the full list. `Read-Choice` reads a typed choice + Enter
   (the 8 s start timeout stops once a key is typed); '' plays.
   6.26 ("found, not fetched"): `Ensure-Dlss` looks first in `Sl-Known`
   (`VICTORY_STREAMLINE_DIR` set before Try, config `sl_dir`, Try's own),
   then, when none is new enough, in `Sl-Places` (nvidia-sdk in `$Src`,
   `C:\dev\sdk\streamline-sdk-<tag>`, the game's folder, and
   `nvidia-sdk\Streamline\bin\x64` / `streamline*\bin\x64` one level under
   the user folders, the game's parent, `$TryHome` and every fixed drive).
   Used in place when all 8 DLLs are there, NVIDIA-signed, and
   sl.interposer.dll's file version >= `$SlTag`; older: `Get-Streamline`
   into `Streamline.new`, swapped in whole; failed: the older one meanwhile.
   6.25 ("the world's light"; nv's ambient estimate, our cutscene look):
   - `VICTORY_CUTSCENE_LOOK` (**LOOK**; unset auto, `world`, `ours`): auto
     counts the title's upscaler resets (camera cuts; noted once a frame by
     `hle::dlss`, `hle::fidelityfx` or `hle::ffx`): 3 in 20 s turn on our
     constant fill light (`VICTORY_AMBIENT`, as 6.24), 15 s without one turn
     on the world look again. A cut restarts the estimate's slow mean.
   - World look (`victory_gpu::ambient::estimate`, nv's): after
     `CS_CacheScreenSpaceRadiance` (keep, mean), `CS_ScreenProbes_TemporalResolve`
     (place, gather, raise), `CS_AccumulateLocalLightInscatteringVolume` (air)
     and `CS_RayMiss_<8 hex>_` (reflections, opaque layer;
     `VICTORY_AMBIENT_SPECULAR_ALPHA=1` both). `VICTORY_AMBIENT_ESTIMATE`
     (0.8), `VICTORY_AMBIENT_AIR` (1.5), `VICTORY_AMBIENT_SPECULAR=<share>[,<sky>]`
     (1), `VICTORY_AMBIENT_ONCE=1`, `VICTORY_AMBIENT_SHADE`; `VICTORY_AMBIENT=off`
     turns both looks off.
   - `VICTORY_EARLY_DEPTH=1` (Try sets it; nv 41b4a05): DB_SHADER_CONTROL bit
     12 gives DXIL's ForceEarlyDepthStencil where the program exports no
     depth or stencil. `VICTORY_LEAVE_OUT` (unset:
     `02384a60fda6f2d0,731a39171b568298`; `none`). `VICTORY_THREAD_TICKET`
     (=0 off; nv 854f57f).
   - Targets room (edsuuu 7e81806): measured 300 frames after the first
     frame of 500+ draws; peak + max(peak/4, 384 MB) + 256 MB per budget
     overrun, at most the old rule. `VICTORY_CARD_ROOM_FOR_TARGETS_MB` fixes it.
   - Menu: after a run (and at start, 8 s, then 1) a cmd-style list read by
     `[Console]::ReadKey` (Read-Host when input is redirected): 1 / Enter,
     modes 2-6 (`Set-Mode`), 7 `Reset-Recommended`, 8 the full table, U, P,
     Q. `VICTORY_TRY_NO_MENU=1`: none at start. `dlss_dl` keeps
     `failed:<unix seconds>` and retries after 6 h.
   6.23 ("every shader"; the "do not translate" count of prepare):
   - ACCC containers (driver-compiled FidelityFX) translate: DXIL-or-ACCC in
     `emit::translate_plain`, `pipeline::translate`, the spilled path,
     `container_walks`, and the app's survey and translate (avm 66afb2b4).
   - cool_fox 66afb2b (shader half: src_scc, v_dot4, fmac_f16, a16 atomics,
     made-raw descriptors, s_bfm sampler words, s_mov_b64), 389db9e (counter
     texture store), 26a801e (immediate samplers by comparison mode),
     caec503 (LOD query coordinate undef), 13e600d (f16 inline constants in
     `wave::simulate`).
   - Structured access of unknown stride through a table view: stride from
     `SLOT_TABLE_STRIDES` (`slot_word(src, true)`), raw at index*stride+off;
     emitted only where it was refused, so other translations are unchanged.
   - App scan: carriers bounded to the container's size (sp #114), links
     followed, real paths once, packs/exes first (sp #134); sp's scan tests.
   - prepare: reasons tallied (`translate failed` / `translate skipped`, top
     25, the rest to stderr), `$VICTORY_DUMP_FAILED/prepare-<hash16>.dxbc`
     (400 at most); `shader_cache::prepare` and `d3d12x::prepare_shader`
     return `Result<(), String>`. `Prepared` counts recipes not made by why.
   - Not taken: cool_fox b558934 (RT builder only), cb1468d/9a4aa8d/bd8b4f5/
     d2f2397 (SCC branch series), 776b73e (view addresses), 87ed9d3 (WARP).
   6.22 ("from ScarlettPort"; ZigZagBoi1/ScarlettPort up to 0.5.3, ported
   onto our base by hand where it did not apply; messages name the commit):
   - Defaults (each with its way back): `VICTORY_WAVE_PAIRS_ONLY` = the two
     `WATER_PAIRS` hashes and `VICTORY_WAVE_PAIRS_FP32=non-ieee-rne-ftz`
     (`=none` / `=off`); `VICTORY_CARD_BIG_ROOM` 256 MB for copies of 16 MiB
     and more (`=0`); `VICTORY_GUARD_SINK` and `VICTORY_LOADS_WIDE` (shader
     passes `GUARDS`/`WIDE`, also with `VICTORY_SHADER_OPT=0`; `=0`);
     `VICTORY_SAME_HOST_HEAPS` (`=0`); `VICTORY_SPREAD_LEAN` (fx a7ee5df;
     `=0`); `VICTORY_MESH_MAKE_ONE` on AMD hosts (`=0` off, `=1` every card);
     a save of the pipeline library beside a mapped one goes to
     `host-pipelines.next` (`VICTORY_LIBRARY_REPLACE_MAPPED=1`);
     `VICTORY_UNWIND_CHECKS` (`=0`).
   - try.ps1: `VICTORY_PIPELINE_WAIT_MS=0` (250 before; compute still waits,
     `VICTORY_PIPELINE_COMPUTE_WAIT`); `VICTORY_CARD_BUFFERS_MIN_KB=64` unless
     the hybrid, plain mode, or a user's `_MIN_MB`/`_MIN_KB`/`VICTORY_INPLACE`
     (`_MIN_KB=0` off). `shadow::card_limits` reads `_MIN_KB` again (fx
     c36971b2).
   - Taken: sp f059bbf (saves: leftovers removed once, no wait), 50a9f7c,
     2387034, fbf382d (write-watch), e5faadb (loose windows `RwLock`),
     1e72650 (same host heaps; Belobug #326, #332, #351), 56a926f (refusal
     reasons), 6d836e8 (AMD mesh, mapped library), f2bef5f (Belobug #324 size
     query, #343 stack walk), bc40787 (`oom::Reporting` under copystat's
     allocator).
   - Not taken: sp 4c03a59's upscaler fallback (ours takes `ffxDispatch`
     itself), 5b0288d and 2703ed0 (sp's profiler), cfc3661 (sp's warm),
     f2bef5f #295 (KBM), 1deb0be (our reserved textures skip the pool already);
     cool_fox CARD_STEPS, UPLOAD_AHEAD, SHOW_AFTER, WRITE_SITES, RELAY_COPY
     (built on cool_fox modules ours lacks), MAP_THREADS, THIN; fx 3461f52.
   - Diagnostics builds compile again (`Step::AfterTextures` in the step
     timer); the card memory line splits buffers, windows and BAR windows.
   6.21 ("from the 4070 fork"; fxceless/victory-fork up to 9a9091d):
   - Taken by cherry-pick (fx author kept; messages `fx <commit>: ...`):
     d9049b4, 59750d7, 3382ad9, 2c6bc78, #1376, #1380, a42b627 (#1312),
     30c1466, 0a40a00, 4c60466, b1d777a (by hand where it met ours), 62a8d51,
     f9e0371, 3221ac4, 0e006f0, 81d54b0 (#1407), 8a32d0b (#1419), 3ab5c12,
     b63b465, 50c79c3, e3307c7, a918163, 4966cd6, 17533c5, bcc5421, 1170216,
     1320dfd, 9a9091d. Kept ours: `merge_runs()` 16384, the workers in
     `pipeline/make.rs`, `bounds_pipeline`.
   - `pipeline::builder_runs`: only with `VICTORY_BVH_BUILDER=1` (fx 4966cd6
     made it the default; 0.2-3 fps on his 4070); `intersect::no_hit` on
     while it is off, and `lower::descriptor_data` reads 0 then too (the five
     RT computes translate without `VICTORY_SKIP_RT`).
   - `make::water_light_left_out`: not when `VICTORY_WAVE_PAIRS_ONLY` lists
     both `WATER_LIGHT` hashes (the paired form).
   - try.ps1: `VICTORY_DENY_PIPELINES_FILE` = rt-off-hashes.txt.
   - Skipped: 3461f52 (GPU detile, needs fx's staging pool), ff1776b (ours has
     `VICTORY_DUMP_FAILED`), a7ee5df (`VICTORY_SPREAD_LEAN`, off in fx).
   6.20 ("a clean log"; every WARN of the 6.18 report):
   - try.ps1 `Set-AllOut`: on cards under 8 GB, `VICTORY_CARD_BUFFERS_MB=512`
     (a fifth of the card before) and no 35% BAR windows (20% as normally).
   - `emit::lower::descriptor_data`: with `VICTORY_SKIP_RT=1`, in a program
     with `image_bvh*`, a heap descriptor read only as data reads 0 (the
     five `07c93bfb77fecebb`, `002e93a6cc2aa355`, `0e64f14fae060d04`,
     `96ccb14ef981fd85`, `dbdbf36298727135` translate).
   - `descriptor::host::unaligned_constant`: a constant view off 256 in its
     copy is made on `shadow::exact` (and `buffer_copy` refreshes that copy);
     `own_bytes` also takes raw views whose first dword is off 16 bytes.
   - `draw::host_word` `USER_VIEW_ADDRESS`: a constant view's address.
   - `host::fill_ahead::wrote`: a fill whose submission number is at most
     `FINISHED` is not compared (a landing of unknown flights counted all).
   - hle: `rpcrt4.dll` in `HostPolicy::default`; a refused `GetModuleHandle`
     probe logs at info. Loader: emulated console instructions at info.
   - gpu: deliberate denials and RT-off drops, the 0x1c8/0x218 E_NOTIMPL
     slot and the view heap cap at info; `VICTORY_NATIVE_PSO_LOG=1` dumps
     native pipeline descriptions.
   6.19 ("no light in blocks"; 6.18 did not build: `TARGET_LOOK_AT`):
   - `resource::uav_texture_at` (cool_fox e11479ae, #72): placed unordered
     texture views get a host UAV texture at their address (an existing
     compatible target there, else one made in `PLACED_TEXTURES` and so in
     `TARGETS_AT`); `placed_texture` returns `View::Unordered` for them and
     clears the target/depth/UAV flags of placed shader views, which then
     find it through `alias_of`. `VICTORY_PLACED_UAV=0`: as before.
   - `pipeline::make::water_light_left_out` (nv 35c15608): the water's
     tiled light (5dc3b25986622949, 6f225b641c5b0708) is not built on
     32-lane hosts; `VICTORY_WATER_LIGHT=1` builds it.
   - `VICTORY_TEXTURE_OVER_DROP` (on): adaptive quality drops 3 levels while
     the card is past its budget.
   - `emit::take_form`: `shader_cache::translate_listed` logs translations
     made in an approximate local-wave form. `VICTORY_WAVE_WIDE_FIRST=1`
     (off) tries `widen` before them for groups of one console wave
     (`_NOT=<hash>,...`).
   - Lines: `render targets placed in title memory (6.19)` (copies sharing a
     start address, placed UAV textures), `indirect executions not run
     (6.19)`, `shaders translated approximately (6.19)`.
   - Checked before the push on Linux: `cargo check --all-targets` (and
     `--features diagnostics`) of victory-gpu with `--cfg windows` against
     the windows-rs sources, and victory-shader's tests.
   6.18 ("nothing let go that comes back"; plan and to-do in `scripts/try/PLAN.md`):
   - `VICTORY_SWEEP_BACK_MS` (30,000): a texture the sweep let go and that
     was made again waits that long unused before it may go again; the
     sweep's idle frames are 8 (4 before).
   - `VICTORY_TARGET_SWEEP` (2): the sweep also lets go of render-target
     copies nothing holds, unused 30 s, whose title memory is unmapped (1),
     and those unused two minutes while the card is past its budget (2).
   - `VICTORY_JOIN_HOLD_MS` 1,500 by default (10,000 before).
   - A `texture sweep (6.18)` line beside the memory lines, with the
     placeholders by kind.
   6.17 ("no wasted processor"; plan and to-do in `scripts/try/PLAN.md`):
   - `console_insn::plan`: a `monitorx`/`mwaitx` whose first two bytes lie
     across an eight-byte word keeps its `0F` and gets `1F C0` (`nop eax`)
     in one store; places that cannot be patched are not tried again
     (`UNPATCHABLE`), the first reasons are logged.
   - `VICTORY_HEAP_AUDIT_SPREAD` (on): the heap pass over buffer views
     looks at those written plus a sixteenth of all (`audit_next`) every
     pass, not all of them every sixteenth pass (Kelvaroq 0055).
   - Try sets `VICTORY_SYNC_BESIDE=1` (config `beside`; second in the
     lost-card chain, after all-out); mirror `MfDev0/CATez` first.
   6.16 ("room for windows"; plan and to-do in `scripts/try/PLAN.md`):
   - `VICTORY_WINDOWS_ROOM` (on): the texture queue's room counts what
     windows outside buffers may still take on the card (BAR and card
     copies, at most `VICTORY_WINDOWS_ROOM_MB`, a quarter of the budget);
     windows go to the card up to 96 % of it (80 % with it off).
   - `VICTORY_TEXTURE_RUSH_GAP_S` (30): the rush rests after it ends;
     textures `unrush` made due are uploaded whole.
   - `VICTORY_SHADER_CACHE_KEEP` (2): shader translations in a folder per
     build key (`b-<hex>`), older builds' removed 90 s into a run.
   - The card meter (`VICTORY_CARD_METER`, Try: on) works without the frame
     log and writes a line by kind of list. Try: MAX under 8 GB of card
     memory, textures 94 % and card pressure 95/98; `TIDY` and the disk
     tidy (config `tidy`).
   6.15 ("new things fast"; plan and to-do in `scripts/try/PLAN.md`):
   - `VICTORY_LIBRARY_STORE_LATER` (on; cool_fox 4a028f31),
     `VICTORY_LIBRARY_SAVE_THREAD` (on), `VICTORY_LIBRARY_SAVE_S` 300 in the
     world whatever the memory.
   - `VICTORY_STAND_IN_FIRST` (on; after Kelvaroq 0050); Try: stand-ins on
     their own switch (config `standin`), `VICTORY_PIPELINE_WAIT_MS=250`.
   - Panicking pipeline builds refused (cool_fox 744c09ba); aliased
     textures held while copied (cool_fox 94f35b7f).
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
