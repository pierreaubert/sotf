# rdsd2pcm
A pure Rust library for converting DSD to PCM. Logging implemented via [log](https://crates.io/crates/log) crate. Reads DSD from stdin or file, writes PCM to stdout or file.

Doc comments are present, so you can see documentation with `cargo doc --open`.

**Note**, this repo was previously a rust binary with a wrapper around the dsd2pcm library. For a more full-featured version of a command line binary, see [dsd2dxd](https://github.com/clone206/dsd2dxd/), which now uses this library. See its main.rs for an example of use.