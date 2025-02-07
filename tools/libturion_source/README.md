# libturion_source

This is a replacement library in Rust for libBambuSource.so that only support "local" schema.

This was tested on a P1S in LAN mode.

## Usage

Build using `cargo build --release`, grab `libturion_source.so` in `target/release/` and replace the `libBambuSource.so` file in `~/.config/BambuStudio/plugins/` or `~/.config/OrcaSlicer/plugins/` depending on the tool you use.

NOTE: I wasn't able to get OrcaSlicer to work on my end even with the official library, but it should work fine (related to issue [#6585](https://github.com/SoftFever/OrcaSlicer/issues/6585))

## Licensing

This software is licensed under the terms of the LGPLv3.

You can find a copy of the license in the [LICENSE file](LICENSE).
