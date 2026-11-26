VIctory Try 6.5 (tricks from big games)
- Streaming budget per frame: big games give texture streaming a fixed slice of every frame. Now the world's texture loading takes at most 35% of a frame. In your 6.2 log one pass took 750 ms of a 900 ms frame while catching up. Loading screens and the 6.4 curtain still load at full speed
- Steadier texture quality: quality now switches with a gap (blur from 85% card use, sharp again under 80%), like games switch detail levels, instead of flipping back and forth every tenth of a second and loading the same textures again
- Built-in profiler: the log now shows where the texture loading time goes (checks, memory queries, making textures, recording uploads). That tells us exactly what to move to other CPU cores next
- Everything from 6.4: the world curtain, the priority order, reused upload buffers, shader cache read ahead, people less bright (A: normal / low / high / off)
- Settings: VICTORY_TEXTURE_FRAME_SHARE (35, 0 off)
