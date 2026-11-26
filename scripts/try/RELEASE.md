VIctory Try 5.1
- The 6 compute pipelines the RTX 4060 refused every run (64-lane waves) are now made as two 32-lane halves
- Halves meet only where every lane runs, so no card freeze; their dispatches are no longer dropped
- Lane permutes, wave-wide branches, 16-bit wave sums and lane masks handled per half in that form
- Based on the split-wave fixes from the open VIctory pull requests (#1890, #233)
