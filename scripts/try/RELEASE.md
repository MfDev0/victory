VIctory Try 6.7 (new game or continue, smooth hair and shadows, less waiting)
- Before the game loads you now pick: Enter = continue your save, N = NEW GAME from the very beginning (mission 1, even if your save is further on), the same as the first time you played. Your save is never deleted. It is renamed and kept, and J in the menu brings it back. J also turns the question off
- Hair and soft shadows: the Xbox upscaler builds each picture from several frames. Until now we stood in for it with a plain one-frame stretch, so hair looked dotted or see-through and shadow edges crawled. The new stand-in does what the real one does. It takes off the camera jitter, moves the frames already built along the game's own motion, rejects what no longer fits, and blends. It also sharpens a little. T switches it off and on
- Less waiting on the card: up to 384 batches of work can be in flight before the CPU waits (256 before; in 6.6 this wait cost 75-100 ms a frame). It drops back to 256 by itself if the slots run out
- Card results fetched inside their own flight (6.6) now have their own switch. In the 6.6 report this feature never turned on, because an older lost-card setting was holding it off
- More buffers are compared on the card instead of the CPU (2 MB and up, was 4)
- The game's AMD upscaler DLL is no longer refused when it is looked up while it is still being loaded
- Settings: VICTORY_UPSCALE_TEMPORAL=0, VICTORY_UPSCALE_FEEDBACK (10), VICTORY_UPSCALE_SHARPEN (20), VICTORY_TRY_SAVE=continue|new, VICTORY_LATE_WAIT_LISTS (384 from Try)
