VIctory Try 6.4
- Load before you join: when a loading screen ends, the world waits behind a "curtain" (up to 10 s) and loads its textures first, the way the Series S keeps its loading screen up until the first shot is streamed. The first seconds run slowly, then the world shows up whole instead of filling in over a minute
- New priority order: textures whose data just arrived come first, then the queue takes turns between the newest (what the game streams for where you look) and the oldest waiting (first come, first served, so nothing waits forever), with 3 small textures for every big one: a small texture is cheap and colours a whole surface
- Faster texture uploads: buffers are reused from a pool instead of being created for every texture (59,000 of them were created in your last 10 minutes)
- Shader cache read into memory at startup while the PC has RAM to spare, so the loading screen doesn't wait on the disk (pipelines took 2.6-4.5 s there before)
- Resizable BAR uploads back on (they were switched off together with other things after a lost card long ago). Turns itself off again if the card is lost
- People less bright: ambient light 0.45 instead of 0.6. A cycles normal / low (0.3) / high (0.6) / off
- Settings: VICTORY_JOIN_HOLD_MS (10000, 0 off), VICTORY_TEXTURE_PRIORITY=0, VICTORY_STAGING_POOL_MB, VICTORY_LIBRARY_PREFETCH=0, VICTORY_GPU_UPLOAD=0
