VIctory Try 5.2
- Fixes from codec_d's cool_fox (25 fps on NVIDIA) ported into our fork
- Transmission prefilter shader 2da4beb3 now translates: no more black squares on eyes, glass and alpha
- Faces no longer deform: 16-bit data of integer buffer views keeps its bits
- Draws no longer lose their descriptor tables: slots come back as soon as the GPU is done
- One bad indirect record no longer blanks every draw after it
- More 64-lane shaders lowered for 32-lane NVIDIA cards; per-heap buffer refresh every frame
