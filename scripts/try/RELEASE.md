VIctory Try 6.1
- Update button fixed for good: U now switches to the new version in the same window, with a clear "UPDATING" line, and the new version starts from clean settings (it no longer inherits the old version's)
- The version is now everywhere: the window title, the game output window, the first line of the game log, the report zip name, and try-version.txt inside the zip
- Loading screens pre-load the world like the console does: while a loading screen, menu or fade is up, textures get a big upload budget (up to 768 MB and 1.2 s a frame), so the world opens with its textures there instead of popping in over the first minute
- Less "PS3 look" at the start: textures keep full detail until the card is 85 % full (it was 70 %; your 6 GB card sat at 73 %, so people and buildings started blurred)
- Blurred textures get their full detail back 4x faster (1,024 a frame, was 256)
- The game's own file streaming (DirectStorage reads and decompression) runs at above-normal priority, as the console's decompression hardware runs beside its CPU, so high-detail models arrive sooner
- Everything from 6.0 stays (all CPU threads, high-priority card queues, the CPU graphics helper, the tuned build)
