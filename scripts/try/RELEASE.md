VIctory Try 5.6
- Found the main cause of black textures: 1,448 big textures (2.9 GB) never got their memory because the memory step ran before the texture was read. Fixed
- Texture quality for 6 and 8 GB cards: the largest level of big textures is skipped (like "medium" in PC games), so about 3x more textures fit
- Every big texture now only takes memory for the levels it actually shows
- Faster texture memory setup (one call per texture instead of one per level)
- Textures still load (blurry) when the card is slightly over its budget, instead of black
- Auto Series S now reads your card's memory from Windows, so it works on the first run
