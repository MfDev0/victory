# CAtez

![CAtez](docs/media/catez.gif)

## Status

| Stage | State |
|-------|-------|
| Booted to main menu | completed |
| Entering the game | in progress (now on a base that has rendered the world on NVIDIA) |

huss huss huss huss huss huss

## nvidia/: the VIctory-NVIDIA Try build

`nvidia/` holds trustno1-coder's VIctory-NVIDIA (main at `9ce3cf9`, with its
mouse and keyboard controls) plus the fixes testers found working, on by
default: black faces (#669), sky shine (#393), wide dispatch (#1005), movies
as upstream. On top of that:
- photo mode saving to `Downloads\VIctory-Photos`;
- a trail fix (DLSS model K);
- a crash fix for 8 GB cards in Lucia's robbery;
- a first start fix: the black face fix waits while shaders compile, then turns itself on;
- more of the upstream pull requests;
- FG UNLOCK (3x-6x on RTX 20/30/40, from MfDev0/dlss-unlocked-);
- a crash report zip and a tester menu.

It is a separate workspace, and this repository's own engine above is unchanged. To
build it: `cd nvidia`, then `.\BUILD_VICTORY.cmd` (PowerShell needs the `.\`). To test: `.\TESTER_MENU.cmd`. A newer clone in another folder asks whether to replace the older one (see `nvidia/TRY-NOTES.md`).
See [nvidia/TRY-NOTES.md](nvidia/TRY-NOTES.md).

## What this is

A fork of [VIctory](https://github.com/Marvisak/VIctory), which runs the Xbox
Series build of Grand Theft Auto VI on Windows from files the user supplies.
**It contains no game files, code or assets.** It is not affiliated with or
endorsed by Rockstar Games or Take-Two Interactive.

From VIctory Try 3.0, this branch stands on
[fxceless/victory-gta6](https://github.com/fxceless/victory-gta6). That fork
renders the lit prologue (characters, textures, sky, HUD, pad input) at 2-4 fps
on RTX 4070, 4060 Ti, 3080 and 3060 Ti cards with the Series X profile.
Anything the sun does not reach directly is black, because the ray-traced
lighting does not run yet.

Added here on top of it:

- pipeline workers get 64 MB stacks, so the NVIDIA compiler no longer
  overflows on large compute shaders (Peppacodes, upstream #966). Big shaders
  get their own lane, so they don't hold up the small ones;
- large copies of title memory run on several threads;
- `VICTORY_DUMP_FAILED=<dir>` saves the shaders that do not translate;
- the open fixes from victory-gta6 and upstream that were still waiting to
  merge:
  - parked lane masks: the irradiance cache's shading, which lights the
    shadows;
  - 64-bit float conversion;
  - immediate samplers chosen by branches;
  - constant buffer views capped at 64 KiB;
  - a 256-entry pad input history;
  - descriptors read only as data (upstream #1067).

## Running it

Use [VIctory Try](scripts/try/README.md). It builds, sets the switches the
NVIDIA reports found stable, and runs the game:

- `--console series-x`: the Series S profile loses the device before the first
  frame on this base;
- `VICTORY_SHADER_LOOP_LIMIT=256` and `VICTORY_RELAY=2`;
- `VICTORY_SKIP_PIPELINES=eeed82b877b44b5f,b83629b90cd2b7bc`.

Press **F** in Try to switch to the faster, riskier speed mode
(`VICTORY_LATE_WAIT=3 VICTORY_WIDE_BACK=1 VICTORY_CARD_BUFFERS_MIN_MB=1
VICTORY_CARD_BUFFERS_MB=1024`).

The first visit to the world compiles about 700 pipelines and is very slow.
Launch it a second time to get the real frame rate.

## Credits

- [VIctory](https://github.com/Marvisak/VIctory) by Marvisak and its
  contributors, GPL-3.0-or-later. So is this fork.
- [victory-gta6](https://github.com/fxceless/victory-gta6) by fxceless and its
  contributors (GPL-3.0-or-later). This fork builds on its tree, and its
  history is kept as a merge parent.
- The authors of each ported change, kept as the commit authors.
