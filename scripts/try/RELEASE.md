VIctory Try 6.11 "no disk stutter" (fixes from lepotane, AvatarManz and the others ported into this fork's engine)
- Shader stutter: the compiled pipelines (the 773 MB pipeline library) are read into memory before play on 16 GB PCs too. In the 6.7 report this was skipped ("too little memory free": it wanted 4 GB spare on top), so every pipeline the game needed in play was read from the disk page by page (seconds each). Now 1 GB spare is enough, and with less the front of the file is read
- Once per build, with the game closed, every shader in the game's files is translated into the cache (as AvatarManz's "victory shaders" does): a pipeline met for the first time in play then only needs the driver's compile. The first time takes 10-30 minutes; stopping it is fine. VICTORY_TRY_ALL_SHADERS=0 skips it
- Speed: parts of the game's submissions that follow one another go to the card as one (VICTORY_JOIN_PARTS, in the Y set; off by itself if the card is lost), and the copy-back work is cut into 256 KB pieces instead of 1 MB so all 12 threads share it (lepotane/VIctory-leonida)
- Picture: anisotropic samplers with point mips stay so on cards that have it (AvatarManz 011735ff; the RTX 4050 does), instead of blending between texture sizes the console does not
- The "check" test of the repository passes again (Rust sources may be up to 1 MB)

VIctory Try 6.10 "own engine + AMD upscaler" (this fork's engine is the base again; the best fixes of the others come into it)
- This fork's engine is the default again, for good. AvatarManz's and the 8 GB engine stay on N only for comparing
- The game's upscale (Series S renders 1280x720 and scales to 1080p) is now made by AMD's real PC FidelityFX upscaler, as on Belobug's engine (PortWatch #1, from DexNom): FSR 3.1 on NVIDIA cards, FSR 4 on new Radeons. Lit and reflecting surfaces should no longer turn into blocks, hair and edges stay sharp. VIctory Try downloads AMD's signed file once (57 MB from AMD's GitHub) and checks its signature every run
- T switches the upscaler: FSR (default) / own / off. If AMD's upscaler cannot run a frame, VIctory's own makes it (never a black screen)
- VIctory's own upscaler (T = own) is sharper and no longer draws the low-resolution grid into bright surfaces
- Shaders AMD's compiler made, and the game's debug shaders stored unpacked, are read now (from AvatarManz)
- The log says which upscaler ran, which AMD versions your card was offered, and how many frames carried the camera jitter the upscaler needs
