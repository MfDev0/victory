# dlss-unlocked (MfDev0)

A copy of [MfDev0/dlss-unlocked-](https://github.com/MfDev0/dlss-unlocked-)
at `b20dda7`: a portable 3x/4x/5x/6x DLSS frame generation selector for
VIctory with [MFG Unlocker](https://github.com/dashdogy/MFG-Unlocker),
tested on an RTX 4050 Laptop GPU. `START-HERE.cmd` and `README.txt` are as
they were published. No binaries are included.

This build has the same thing built in:

- `UNLOCK_FG.cmd` (repository root) offers the same picks and saves them as
  the F1 menu does, so a FRAME GENERATION value saved earlier in F1 no longer
  overrides the multiplier picked. START-HERE.cmd passes `--framegen`, which
  a value saved in F1 overrides.
- `victory play` turns MFG Unlocker on by itself when 3x to 6x is asked on an
  RTX 20, 30 or 40 (F1, FG UNLOCK: AUTO; `VICTORY_FG_UNLOCK=0` or FG UNLOCK
  OFF stops it). It also writes `RTXMFG-Universal.json` and `RTXMFG-UI.ini`
  beside victory.exe with the values START-HERE.cmd writes, at the multiplier
  asked (`VICTORY_MFG_CONFIG=keep` leaves them alone).

START-HERE.cmd still works as it is: put it beside victory.exe.
