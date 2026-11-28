# Try build on VIctory-NVIDIA

This folder is [trustno1-coder/VIctory-NVIDIA](https://github.com/trustno1-coder/VIctory-NVIDIA)
at `63b902f` (tag v2026.11.28.01, the newest when this was made). The fixes,
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
git clone -b arena/59fabb00-victory https://github.com/MfDev0/victory victory-try-2
cd victory-try-2\nvidia
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

| What you see | Change | Switch |
|---|---|---|
| Diner/coffee-shop waitress and bartender: face specks, blue or black | PR #33 with the fix from its review (below) | F1 DINER FACE FIX, `VICTORY_PRELUDE_RECHECK=0` |
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
| Draws dropped in busy outdoor scenes on 1M-descriptor heaps | PR #1007: 458,752 dynamic slots | `VICTORY_DYNAMIC_SLOTS=262144` is the old value |

**The diner fix.** #33 skips a texture's base upload when the host wrote the
texture after the list that samples it was recorded. In #33 as posted, a list
that uploads a texture and also copies part of it found its own copy and
cancelled its own upload. Testers then saw black NPC faces and black world
textures (dcastell1 and JoeY54 in #33's review). Here the check only counts
copies from lists that were already submitted (`HOST_WRITTEN_SUBMITTED`,
filled in `Host::submit` after the list's own uploads are decided), as
dcastell1 proposed. Issue #1058 (prologue NPC faces with the wrong texture)
looks like the same problem.

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
until you turn it on. Turn one on in **F1** (the `TRY PR ...` rows), in
`TESTER_MENU.cmd` > `T`, or with `--set NAME=VALUE`. Each needs a restart.

| F1 row | Pull request | Switch (on) | What it changes |
|---|---|---|---|
| TRY PR 669: BLACK FACE ADMIT | #669 | `VICTORY_FACE_ADMIT=1` | When video memory is full, a face texture that was never uploaded is still let in, up to 80% of the card. Headroom: `VICTORY_FACE_HEADROOM_MB` (256), `VICTORY_TEXTURE_FIRST_UPLOAD_MB` (128). Try it if faces draw black. |
| TRY PR 669: MOVIE FG PAUSE | #669 | `VICTORY_MOVIE_FG_GATE=1` | DLSS frame generation pauses while a Bink movie plays, so its black bars don't flicker. FSR 3 is not paused, as in #669. |
| TRY PR 1005: WIDE DISPATCH | #1005 | `VICTORY_INDIRECT_WIDE_NVIDIA=1` | Indirect dispatches wider than 65,535 groups run in one go on NVIDIA too, as on AMD. D3D12 does not allow more than 65,535, so this may fail on your driver. |
| TRY PR 1008: RT FAULT PAGES | #1008 | `VICTORY_BVH_FAULTS_ALL=16384` | With RAY TRACING on, up to 16,384 faulted pages are copied for the BVH (the old limit is 2,048). |
| TRY PR 982: PAD KEYS | #982 (keymap only) | `VICTORY_KEYMAP=a=20,lt=A2` | The keyboard pad's keys can be changed. The F1 choice puts A on Space and LT on left Ctrl. Any `control=hexkey` list works, for example `b=1B` (controls: a b x y menu view up down left right lb rb ls rs lt rt ls_up ls_down ls_left ls_right rs_up rs_down rs_left rs_right). It does nothing while GTA V KEYS AND MOUSE is on. |
| LANGUAGE | #1056 | `VICTORY_MENU_LANGUAGE=zh-CN` / `zh-TW` / `es` / `auto` | F1 and F3 in Chinese or Spanish, using the Windows fonts. English stays the default. This one is merged in full: it is only the menu. |

Not taken at all: #982's Tauri launcher (4,500 lines of new crates; crates.io
was not reachable to check them) and #1056's bundled fonts (it uses the fonts
Windows already has).

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

`T` is a submenu with the switches above:

- 1 to 6: the left-out PRs and Discord. Each press flips one and shows ON
  or OFF. They are saved the way F1 saves them, so F1 and the cmd always
  agree.
- 7: the menu language.
- B: back.
