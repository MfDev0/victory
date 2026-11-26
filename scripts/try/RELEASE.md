VIctory Try 6.10 "own engine + AMD upscaler" (this fork's engine is the base again; the best fixes of the others come into it)
- This fork's engine is the default again, for good. AvatarManz's and the 8 GB engine stay on N only for comparing
- The game's upscale (Series S renders 1280x720 and scales to 1080p) is now made by AMD's real PC FidelityFX upscaler, as on Belobug's engine (PortWatch #1, from DexNom): FSR 3.1 on NVIDIA cards, FSR 4 on new Radeons. Lit and reflecting surfaces should no longer turn into blocks, hair and edges stay sharp. VIctory Try downloads AMD's signed file once (57 MB from AMD's GitHub) and checks its signature every run
- T switches the upscaler: FSR (default) / own / off. If AMD's upscaler cannot run a frame, VIctory's own makes it (never a black screen)
- VIctory's own upscaler (T = own) is sharper and no longer draws the low-resolution grid into bright surfaces
- Shaders AMD's compiler made, and the game's debug shaders stored unpacked, are read now (from AvatarManz)
- The log says which upscaler ran, which AMD versions your card was offered, and how many frames carried the camera jitter the upscaler needs
