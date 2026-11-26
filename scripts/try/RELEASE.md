VIctory Try 6.3
- Loading screens now really pre-load the world. Your 6.2 log showed the 6.1/6.2 pre-loading never ran on the loading screen itself (it only got 12 ms a frame there), so you entered the world with about 5,800 textures still waiting. Now each loading-screen frame spends 200 ms loading textures, about 16x more than before
- No more random 1-2 second freezes in the world: the loading-screen budget was switching on in normal world frames too. It now needs 30 frames of loading screen in a row
- Textures loaded behind a loading screen keep full detail (the "picture first" mode only runs in the world now)
- Textures the "picture first" mode made soft now really get their detail back. In 6.2 they stayed soft because the card was always nearly full
- Less RAM churn: the shader cache (770 MB) was saved again 8 times in 5 minutes of play. When the PC is short of memory it now saves every 3 minutes (and always on exit)
- Shaders were already 100% loaded from the cache before the world (2,068 of 2,068 found, 2,516 ready-made pipelines), so that part is done
- Settings: VICTORY_LOADING_MS (200), VICTORY_LOADING_FRAMES (30), VICTORY_LIBRARY_SAVE_S (180), VICTORY_TEXTURE_RUSH=0
