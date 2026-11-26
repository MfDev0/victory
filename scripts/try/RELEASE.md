VIctory Try 5.3
- Upload buffers are reused from a pool instead of made and freed for every copy: fewer driver calls each frame
- Memory guard with 3 levels (normal, getting short, nearly out) from free RAM, commit and the card's budget
- Idle copies are let go sooner when memory runs short, so the card does not overflow into slow PC memory
- After a refused allocation: kept buffers are freed and it is asked again, and texture staging waits 500 ms instead of a stutter burst
- The log shows the memory level and how many upload buffers were reused
