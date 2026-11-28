# Try build on VIctory-NVIDIA

This folder is [trustno1-coder/VIctory-NVIDIA](https://github.com/trustno1-coder/VIctory-NVIDIA)
at `63b902f` (tag v2026.11.28.01, the newest when this was made). The fixes,
pull requests and tester features below are added on top. None of it was
built or run here: it is source for you to build (`BUILD_VICTORY.cmd`) and
test.

## Build and start

```bat
BUILD_VICTORY.cmd
TESTER_MENU.cmd
```

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

## Not taken

- PR #669 (VRAM first-upload admission): untested, and its one review was
  done with the feature off. It also changes Bink video and the instruction
  translator.
- PR #1005: NVIDIA dispatches wider than 65,535 groups are past what D3D12
  allows.
- PRs #1008 (ray tracing only), #982 and #1056 (large launcher and overlay
  rewrites).
