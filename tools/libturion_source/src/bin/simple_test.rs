// Copyright 2025 Mary Guillemard
// SPDX-License-Identifier: LGPL-3.0

use turion_source::*;

fn main() {
    let raw_url = std::env::args().nth(1).unwrap();

    let local_settings = LocalSettings::from_url(raw_url.as_str()).unwrap();
    eprintln!("{local_settings:?}");

    let mut tunnel = LocalTunnel::new(local_settings);

    tunnel.open().unwrap();
    tunnel.start(0x3000).unwrap();

    let mut sample = BambuSample {
        buffer: std::ptr::null_mut(),
        itrack: 0,
        size: 0,
        flags: 0,
        decode_time: 0,
    };

    loop {
        match tunnel.read_sample(&mut sample) {
            Ok(_) => {
                eprintln!("Sample: {sample:?}")
            }
            Err(e) => {
                eprintln!("{e:?}")
            }
        }
    }
}
