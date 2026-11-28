# Try build on VIctory-NVIDIA

This folder is [trustno1-coder/VIctory-NVIDIA](https://github.com/trustno1-coder/VIctory-NVIDIA)
at `9ce3cf9` (its main branch on 2026-11-24, with the mouse and keyboard
controls). The fixes,
pull requests and tester features below are added on top. None of it was
built or run here: it is source for you to build (`BUILD_VICTORY.cmd`) and
test. Everything (the menu, Discord) is built into `victory.exe`.

## Build and start

In PowerShell (it needs the `.\` in front), or double-click the files:

```powershell
.\BUILD_VICTORY.cmd
.\TESTER_MENU.cmd
```

### Trying a newer version

Clone it into a new folder, then build it there:

```powershell
cd $HOME
git clone -b arena/59fabb00-victory https://github.com/MfDev0/victory victory-try-6
cd victory-try-6\nvidia
.\BUILD_VICTORY.cmd
```

`BUILD_VICTORY.cmd` remembers where the project was built last
(`%LOCALAPPDATA%\victory\try-project.txt`). A copy made before that is
found at `%USERPROFILE%\victory-try`. When it finds an older copy, it shows
its folder and version, then asks:

- **1 Replace.** The older copy's `target` build folder and its downloaded
  `D3D12`, `nvidia-sdk`, `amd-sdk` and `fidelityfx` folders are moved over,
  so the build is faster and nothing is downloaded again. Then the older
  folder is deleted. Your F1 settings, game path, saves, logs, reports and
  NVIDIA's DLSS files are in `%LOCALAPPDATA%\victory`, not in the project,
  so they are kept.
- **2 Keep both.** The new copy is the one remembered from now on.
- **3 Cancel.** Nothing changes.

It never deletes these, and keeps both instead:

- a folder that holds `game_scarlett_master.exe`
- the folder it runs from, or one it is in
- a drive, or your user, Desktop, Documents or Downloads folder

If `victory.exe` is running, it asks you to close it first.

`BUILD_VICTORY.cmd` runs `cargo build --release --locked -p victory` and
copies `victory.exe` into this folder. You need Rust from rustup.rs (it
fetches 1.98.1 by itself) and the Visual Studio Build Tools with "Desktop
development with C++". `BUILD_VICTORY.cmd test` also runs the launcher's
unit tests.

## Fixes

**Tested and working: on, and no longer in F1.** Testers found these work,
so they are on by default. F1 no longer shows them; `TESTER_MENU.cmd` > `T`
still turns each one off or on.

| What you see | Change | Switch (TESTER_MENU T) |
|---|---|---|
| Black faces and objects when video memory is full | PR #669 by Mit1123: a texture that was never uploaded is let in just over the line, up to 80% of the card. Headroom: `VICTORY_FACE_HEADROOM_MB` (256), `VICTORY_TEXTURE_FIRST_UPLOAD_MB` (128) | 1, `VICTORY_BLACK_FACE_FIX=0` |
| Movie cutscenes with DLSS frame generation | As upstream: the setting that worked. SMOOTH (a reset on every movie frame) did not help and PAUSE (#669's) stuttered; both stay in the tester menu | 2, `VICTORY_MOVIE_FG=smooth` / `pause` |
| Something flickers or is missing where the game dispatches wide | PR #1005: indirect dispatches wider than 65,535 groups run in one go on NVIDIA too | 3, `VICTORY_INDIRECT_WIDE_NVIDIA=0` |
| Indoors: blue sky in reflections (blue sheen on skin, eyes, glass; white patches on the prison day-room floor) | PR #393 by tippythetaidum, carrying #63 by wasonidas: the sky's light is bounded at 1x the light around | A, `VICTORY_SKY_BOUND=0` |

**New in this round: on, and in F1 so you can turn them off.**

| What you see | Change | Switch |
|---|---|---|
| Photo mode: "can't save photo" | The game saves a photo by asking the console for a screenshot (`XAppCaptureTakeScreenshot`), and was always told "not supported". Now the next frame is saved, as the game drew it (without F1, the panel or notices), as a full-size PNG in **`Downloads\VIctory-Photos`** (`GTAVI-<date>-<time>.png`). `VICTORY_PHOTOS=<folder>` picks another folder | F1 PHOTO MODE SAVES, T E, `VICTORY_PHOTOS=0` |
| Moving cars and people leave a trail of frames behind them | With F1 DLSS MODEL on DEFAULT, DLSS uses model K, the transformer model, which smears much less than the older CNN models. A model picked in DLSS MODEL is kept. If trails remain with frame generation on, try it once with frame generation off and say which it was | F1 TRAIL FIX, T C, `VICTORY_TRAIL_FIX=0` |
| Lucia's store robbery (the first mission): the game closes on 8 GB cards, "a host command list did not close (0x8007000E)" | Textures stop at 75% of an 8 GB card's memory. That value stopped this crash for a tester in issue #232. Cards with 9 GB or more are not changed | F1 CRASH FIX (8 GB CARDS), T D, `VICTORY_ROOM_SAFE=0` |
| A stutter about every 1.5 seconds, even standing still | PR #340: the memory sweep that drains the card runs every 15 s while memory is not pressed | `VICTORY_SWEEP_EVERY_MS=1500` is the old cadence |
| Hitches at cutscene cuts and busy streaming, card full | PR #1095: a texture pass stops after 128 slots held back for room, not 1,024 | `VICTORY_TEXTURE_HELD_CAP=1024` is the old value |
| DLSS MODEL: a model per mode | PR #1164: F1 DLSS MODEL AUTO (BY MODE): M for performance, L for ultra performance, K for the rest (M and L need newer DLSS files; if AUTO looks wrong, use K) | F1 DLSS MODEL |
| KEY BINDINGS did not bring mouse and keyboard back after a bad `bindings.ini` | PR #1185: the file is read again after KEY BINDINGS writes it | - |
| Mouse and keyboard | Upstream's (ItsMK1/VIctory-MnK-patches): F1 MOUSE AND KEYBOARD, and F1 KEY BINDINGS to change the keys; they are saved in `%LOCALAPPDATA%\victory\bindings.ini` | F1 |

**Back on (version 4 as it was), each with an off switch.** These four were
switched off for a while because a 6 GB card crashed on entering the world.
The real cause was the black face fix in a start that had to compile its
shaders (below), so they are on again. To test one, turn it off in
`TESTER_MENU.cmd` > `T` and see what changes.

| What you see | Change | Switch (TESTER_MENU T, on) |
|---|---|---|
| Green or black textures on 8 GB cards | PR #1179: slot eviction follows the texture line and takes any slot a table ever showed, and a waiting texture shows grey, not black or green | F, `VICTORY_PR1179=0` off |
| Vehicles with doors or panels missing or stretched, matte paint, half decals | PR #1197: a mesh's vertex block is no longer cut off at the end of a 4 MiB window | G, `VICTORY_WINDOW_REACH_KB=0` off |
| The robbery mission: black patches or lines, skipped draws ("dynamic descriptor slots" short, issue #918) | PR #1007: 458,752 dynamic slots, not 262,144 | H, `VICTORY_DYNAMIC_SLOTS=262144` off |
| The sand and seabed by the water: one tile repeated in strips | PR #1183: small unordered-access textures (the terrain's page table) get their host copy with their view | I, `VICTORY_UNORDERED_NOW_KB=0` off |

**FIRST START FIX (new).** A tester found that with the black face fix on, the
game crashed on entering the world ("device removed", or "a host command list
did not close (0x8007000E)" with DLSS out of video memory) in a start that had
to compile its shaders: the first start after a new build, or after another
version ran. One start with the fix off, then the next with it on, played.
Now the black face fix does that by itself, with no restart: it waits for the
first 30 seconds, and while shaders are being compiled (more than 4 a second,
or more than 8 waiting, in the last 20 seconds), then turns itself on. The
log says "black face fix (PR #669): waiting while pipelines are compiled" and
"on now, pipelines have calmed". `TESTER_MENU.cmd` > `T` > J, or
`VICTORY_FACE_WARMUP=0`, turns the wait off.

**The robbery crash is not fully fixed.** Two crashes are reported there. The
first, out of memory on 8 GB cards, is what CRASH FIX (8 GB CARDS) and PR
#1007 address. The second, "device removed (0x887A0007)" after 5 to 7 minutes
of heavy city play, has no known cause upstream yet (issue #232). If yours is
that one, MAKE TESTER REPORT and send the zip.

**Other fixes in this build**

| What you see | Change | Switch |
|---|---|---|
| Ray tracing: compacted structures copied with a stale size (#689) | PR #718 by tharun-37: the size is answered at the builder call. Only with RAY TRACING on; the reporter measured 75 to 67 ms a frame | F1 RT SIZE FIX (on), `VICTORY_BVH_SIZE_ANSWER=0` |
| Crash on 16 GB RAM when the open world streams in (#1049) | PR #1118 by erax4193: spare upload buffers are dropped and the commit is tried again | - |
| Frozen picture after Alt+Enter with FSR 3 frame generation | PR #1132 by adwen-007 | - |
| Ocean height map not read back after its memory is reused | PR #686 by kibagami-nc (port of fxceless#1376) | - |
| Block-compressed 1D textures accepted | PR #1016 by Damokles2 | - |
| Faces with only eyes and teeth | PR #938: the face programs are exempt from the shared-memory bounds | F1 FACE PROGRAMS UNBOUNDED, `VICTORY_LDS_BOUNDS_EXEMPT=0` |
| Floating eyes and teeth on a first start | PR #703: the heads' blend-shape map is made with its pipeline | - |
| "DLSS FRAME GEN: NOT ON THIS CARD" on an RTX 50 | PR #1042 (#1028) | - |
| Wrong lanes on Wave32 hosts | PR #1019 | - |
| Mouse capture fighting window drags | PR #1002 | - |
| Preload/warm ignoring F1 settings | PR #1001 | - |
| "fence 18446744073709551615" on a lost device | PR #993 | - |
| Indirect mesh dispatches with X only | PR #996 | - |
| Depth written and read in the same list | PR #1018 (confirmed on 5070 Ti and 4070 Ti S) | - |
| Aliasing barriers on textures that are not pooled | PR #999 | - |
| Out of memory while staging a texture | PR #986: retried in smaller blocks | - |

The diner face fix (PR #33, `VICTORY_PRELUDE_RECHECK`) was taken out. It did
not fix the diner faces on a tester's PC.

## Features

- **FG UNLOCK, 3x to 6x on RTX 20/30/40.** This is
  [MfDev0/dlss-unlocked-](https://github.com/MfDev0/dlss-unlocked-) built in.
  Its files are copied unchanged into `tools/dlss-unlocked`.
  - When DLSS frame generation is set to 3x or more on an RTX 20, 30 or 40,
    `victory play` turns MFG Unlocker on for that start. It also writes the
    mod's `RTXMFG-Universal.json` and `RTXMFG-UI.ini` beside victory.exe at
    that multiplier, with the values START-HERE.cmd writes.
  - Controls: F1 FG UNLOCK AUTO/ON/OFF; `VICTORY_FG_UNLOCK=0`;
    `VICTORY_MFG_CONFIG=keep` leaves the mod's files alone.
  - `UNLOCK_FG.cmd` is START-HERE.cmd's menu. It saves your pick the way F1
    does, so a FRAME GENERATION value saved in F1 no longer overrides it.
- **Tester report.** `victory report` (or `MAKE_REPORT.cmd`, or F1 MAKE TESTER
  REPORT) writes one zip to `Downloads\VIctory-reports`. It holds:
  - the 3 newest logs and `crashes.log`
  - `victory-settings.txt`
  - MFG Unlocker's `RTXMFG-*` files
  - `summary.txt`: each warning and error once, with a count, plus the frame
    gen, DLSS and crash lines
  - `system.txt`: GPU, driver, CPU, RAM, HAGS, Windows, exe hash
- **Crash report.** `victory play` starts a detached watcher. It makes the zip
  by itself when the game ends with a failure exit code. A normal quit, a
  closed window and F1 restarts don't count. It never runs inside the game.
  `VICTORY_CRASH_REPORT=0`, or F1 CRASH REPORT ZIP OFF, turns it off.
- **GTA V keys and mouse (PR #989).** The mouse turns the camera through the
  keyboard pad's right stick. F2 frees or locks the mouse. Turn it on with
  F1 GTA V KEYS AND MOUSE.
- **TESTER_MENU.cmd.** A cmd-style menu:
  - Series X and S DLSS modes (each pick is saved as F1 saves it)
  - frame gen 2x-6x / unlock
  - preload, report, logs folder
  - DLSS/frame gen check
  - reset F1 settings (kept as `.bak`)

## Frame generation on cards without DLSS frame generation

FSR 3 frame generation, the AMD path used here on cards without DLSS frame
generation, makes one frame per rendered frame. Its interface has no setting
for more, so 3x-6x on those cards is only possible with DLSS frame generation
through MFG Unlocker. MFG Unlocker is someone else's mod
(github.com/dashdogy/MFG-Unlocker, MIT), downloaded from its release and
checked against its published SHA-256. NVIDIA does not support it.

## Left-out pull requests: try them

These pull requests were not taken as they are. Each one is here behind its
own switch, and the switch is **off** by default, so the game behaves as before
until you turn it on. Turn one on in **F1**, in `TESTER_MENU.cmd` > `T`, or
with `--set NAME=VALUE`. Each needs a restart.

| F1 row | Pull request | Switch (on) | What it changes |
|---|---|---|---|
| OUTFIT CHECK (PR 280) | #280 by Mikey970 | `VICTORY_KEEP_SHOWN=checked` | When the game gives a texture slot another texture of the same size, the old one is kept on screen only if their bytes match; otherwise a neutral texture shows until the new one is uploaded. Try it if a character wears the wrong outfit (Lucia in prison clothes in the coffee-shop flashback). |
| NEUTRAL PROBE LIGHT (PR 851) | #851 by t90 | `VICTORY_PROBE_NEUTRAL=0.3` | The game's bounced light is made grey. It removes false red or blue light on skin and walls, and real colour with it. Try it if arms or faces stay light blue with SKY SHINE FIX on. |
| TRY PR 1008: RT FAULT PAGES | #1008 | `VICTORY_BVH_FAULTS_ALL=16384` | With RAY TRACING on, up to 16,384 faulted pages are copied for the BVH (the old limit is 2,048). |
| TRY PR 982: PAD KEYS | #982 (keymap only) | `VICTORY_KEYMAP=a=20,lt=A2` | The keyboard pad's keys can be changed. The F1 choice puts A on Space and LT on left Ctrl. Any `control=hexkey` list works, for example `b=1B` (controls: a b x y menu view up down left right lb rb ls rs lt rt ls_up ls_down ls_left ls_right rs_up rs_down rs_left rs_right). It does nothing while GTA V KEYS AND MOUSE is on. |
| LANGUAGE | #1056 | `VICTORY_MENU_LANGUAGE=zh-CN` / `zh-TW` / `es` / `auto` | F1 and F3 in Chinese or Spanish, using the Windows fonts. English stays the default. This one is merged in full: it is only the menu. |

Not taken yet: #1157 (CPU lock contention, 550 lines across the hottest
paths) and #1173 (pipeline workers throttled under memory pressure, 220
lines): too risky without a Windows build.

Not taken at all: #982's Tauri launcher (4,500 lines of new crates; crates.io
was not reachable to check them) and #1056's bundled fonts (it uses the fonts
Windows already has). Also not taken yet: #1046 (ray tracing builder within
a wait budget, 3,800 lines; it conflicts in five files with what is merged
here, #1008 among them) and #1147 (render-target aliasing, on by default:
too risky without a Windows build to test it). #294 is already in upstream.

**The prologue's order is the game's.** It opens in the prison (Lucia
with the warden); the bar and coffee-shop scenes are her flashback. If Lucia
wears her prison clothes in the flashback instead of her hoodie, that is a
texture shown from before (OUTFIT CHECK above, and BLACK FACE FIX, which lets
first uploads in when the card is full). Please report whether OUTFIT CHECK helps.

## Our touch

- A small cat `=^.^=` on the F1 title line, in `BUILD_VICTORY.cmd`, in
  `TESTER_MENU.cmd` and in the console when the game starts.
- **Discord status.** While the game runs, your Discord profile shows
  **Playing Grand Theft Auto VI** with the cover and the time played. Friends
  see one button, **Play on PC too**. It opens a YouTube video of a cat
  singing Never Gonna Give You Up
  (https://www.youtube.com/watch?v=K7XHy8nppf4).
  - You need the Discord desktop app running.
  - `victory play` starts a small hidden helper that talks to Discord on
    this PC and closes with the game. It is std only and never runs inside
    the game. The pipe code is adapted from github.com/zm0kiemedia/Neptune_VI
    (GPL-3.0).
  - The word after "Playing" is the name of the Discord application. The
    default is Neptune_VI's (`1575009406737846333`). To be sure of the name,
    make your own application called "Grand Theft Auto VI" at
    discord.com/developers and set `VICTORY_DISCORD_APP=<its id>`. If Discord
    refuses the cover from the web, the application's `logo` image is used,
    so upload one under that name.
  - Off: F1 DISCORD STATUS OFF, or `VICTORY_DISCORD=0`.

## TESTER_MENU.cmd > T

`T` is a submenu with the switches above. Each press flips one and shows ON
or OFF. They are saved the way F1 saves them, so F1 and the cmd always agree.

- 1, 2, 3, A: the fixes that work, only here (black face, movies, PR 1005,
  sky shine). 2 goes UPSTREAM, SMOOTH, PAUSE.
- 4, 6, 8, 9: PR 1008, PR 982, PR 280, PR 851 (off by default).
- 5: Discord. 7: the menu language.
- C, D, E: trail fix, crash fix (8 GB cards), photo mode saves (on).
- B: back.
