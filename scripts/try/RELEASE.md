VIctory Try 6.2
- The whole picture first, the detail after (how streaming games do it): while lots of textures are still waiting to load, each one first loads without its two biggest sizes. That is about 1/16 of the work, so the whole scene shows up fast, slightly soft. Once the backlog is gone, every texture gets its full detail back while you play
- What the game draws on screen right now is still loaded first, on the spot; the rest fills in behind it
- Loading screens pre-load the world (from 6.1): while a loading screen or menu is up, textures get a big budget so the world opens ready
- Settings: VICTORY_TEXTURE_RUSH=0 turns the picture-first mode off (1 or 3 changes how many sizes it skips at first)
- The version is in the window title, the game log and the report name, so please send the report from this run
