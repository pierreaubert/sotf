The directory is for non native rust libraries. If it is too complicated to get it to work portably across all platform,
we clone in rust the part of the library we are using.

- bliss audio: removed and ported to rust
- blas: removed and use oxiblas
- ndarray: removed and use oxiblas

Potential similar changes:
- mialloc
- fft to oxifft

Wrappers that are working well:
- lots of macos specific crates
- sqlite
