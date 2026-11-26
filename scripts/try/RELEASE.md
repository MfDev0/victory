VIctory Try 6.9 "AvatarManz engine" (the engine of PortWatch #1 and #2 is now the default)
- The default engine is AvatarManz's, in Belobug's branch: avatarmanz/VIctory-7900XTX (16-22 fps, 45 ms a frame on Series S on an RX 7900 XTX) plus Belobug's three fixes (#1 on PortWatch, about 22 fps): trees and signs stop flickering, texture sizes follow the card's free memory (what lets a 6 GB card hold it), and PC FSR 4.1.2. N switches back to this fork's engine or the 8 GB one
- It runs with AvatarManz's own settings: list calls on eight threads, submissions joined, textures on their own copy queue, fills done on the card. Its five renderer patches are on (Y turns them off)
- No more shaders compiling while you play: with the game closed, every shader of the game is translated once per build, and every pipeline your earlier runs drew with is made. The first time takes a few minutes
- New switches for this engine: B ray tracing (lit rooms and right reflections, about a third slower), X textures (auto, sharp, low), A ambient light, F above 30 fps, Z safe mode
- AvatarManz's engine takes 30-46 GB of RAM plus page file for now. VIctory Try shows how much your PC has before each run. If the game closes by itself, set the page file to System managed (or 49152 MB) and restart
- After a lost card the next run is safer by itself: ray tracing off, then safe mode, then the patches off
- The report zip now also has settings.txt (every setting, RAM, page file, card) and prepare.log
- This fork's engine gets AvatarManz's real output size switch (VICTORY_OUTPUT). The game ignored VICTORY_DISPLAY_MODE
