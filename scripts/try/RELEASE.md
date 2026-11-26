VIctory Try 4.9
- Back to this fork's engine, with AvatarManz's fixes copied in (not his engine)
- No more 400 ms wait for the card every frame (memory changed under the card's writes is merged instead)
- Card kept awake between submissions (it idled 180 ms a frame and lowered its clocks)
- Textures keep showing the old copy until the sharper one is ready (no black pop-in)
- Full 8 GB card: new textures load at low detail instead of staying black/grey (17,000 were waiting)
- Shader fixes: exact packed buffer values (wrong materials), sharper cube-map reflections, vertex-program views declared
- Render targets not set again when unchanged; changed memory ranges joined
- Report lists why draws were not drawn
