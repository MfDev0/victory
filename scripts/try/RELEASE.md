VIctory Try 5.0
- All 3 compute shaders that failed in the last log now translate (f133e35e, 7f05ddab, ceb5eb68)
- 64-lane compute programs get a 32-lane form more often (the RTX 4060 refused 6 pipelines every run)
- Empty buffer views read as empty, so the program that checks for them skips itself (no "view not there" reads)
- Recommended settings switched back on by default for everyone (D key resets them any time)
- Textures the game refills itself: re-upload now logged as info, not as a warning
