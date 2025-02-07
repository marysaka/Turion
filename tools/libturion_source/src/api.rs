// Copyright 2025 Mary Guillemard
// SPDX-License-Identifier: LGPL-3.0

use std::{
    ffi::{c_ulong, CStr},
    io,
    os::raw::{c_char, c_int, c_void},
};

use crate::{BambuSample, LocalSettings, LocalTunnel};

#[derive(Debug)]
#[repr(C)]
pub struct BambuVideoStreamInfo {
    pub stream_type: u32,
    pub sub_type: i32,
    pub width: i32,
    pub height: i32,
    pub frame_rate: i32,
    pub format_type: i32,
    pub format_size: i32,
    pub max_frame_size: i32,
    pub format_bufer: *const c_char,
}

const BAMBU_WOULD_BLOCK_ERROR: c_int = 2;
const BAMBU_GENERIC_ERROR: c_int = 4;

#[no_mangle]
pub unsafe extern "C" fn Bambu_Create(
    handle_out: *mut *mut LocalTunnel,
    path: *const c_char,
) -> c_int {
    let c_str = CStr::from_ptr(path).to_string_lossy();

    let settings = match LocalSettings::from_url(&c_str) {
        Ok(settings) => settings,
        Err(e) => {
            eprintln!("TURION: {e}");
            return BAMBU_GENERIC_ERROR;
        }
    };

    let internal_tunnel = Box::new(LocalTunnel::new(settings));

    *handle_out = Box::leak(internal_tunnel);

    0
}

#[no_mangle]
pub unsafe extern "C" fn Bambu_Destroy(handle: *mut LocalTunnel) {
    /* Recreate and drop the box */
    let _ = Box::from_raw(handle);
}

#[no_mangle]
pub unsafe extern "C" fn Bambu_Open(handle: *mut LocalTunnel) -> c_int {
    if handle.is_null() {
        return -1;
    }

    let tunnel = unsafe { &mut *handle };

    if let Err(e) = tunnel.open() {
        eprintln!("{e:?}");

        return -1;
    }

    0
}

#[no_mangle]
pub unsafe extern "C" fn Bambu_Close(handle: *mut LocalTunnel) {
    if handle.is_null() {
        return;
    }

    let tunnel = unsafe { &mut *handle };

    if let Err(e) = tunnel.close() {
        eprintln!("{e:?}");
    }
}

#[no_mangle]
pub unsafe extern "C" fn Bambu_GetStreamCount(handle: *mut LocalTunnel) -> c_int {
    if handle.is_null() {
        return -1;
    }

    /* This is hardcoded in libBambuSource */
    1
}

#[no_mangle]
pub unsafe extern "C" fn Bambu_GetStreamInfo(
    handle: *mut LocalTunnel,
    _index: i32,
    info: *mut BambuVideoStreamInfo,
) -> c_int {
    if handle.is_null() {
        return -1;
    }

    /* This is mostly hardcoded in libBambuSource */
    *info = BambuVideoStreamInfo {
        stream_type: 0,
        sub_type: 1,
        width: 1280,
        height: 720,
        frame_rate: 1,
        format_type: 2,
        format_size: 0,
        /* XXX: This might be sourced from the first header, check this (unused so do we care?) */
        max_frame_size: 32549,
        format_bufer: core::ptr::null(),
    };

    0
}

#[no_mangle]
pub unsafe extern "C" fn Bambu_StartStreamEx(handle: *mut LocalTunnel, stream_type: i32) -> c_int {
    if handle.is_null() {
        return -1;
    }

    let tunnel = unsafe { &mut *handle };

    if let Err(e) = tunnel.start(stream_type) {
        eprintln!("{e:?}");

        return -1;
    }

    0
}

#[no_mangle]
pub unsafe extern "C" fn Bambu_StartStream(handle: *mut LocalTunnel, video: bool) -> c_int {
    if video {
        Bambu_StartStreamEx(handle, 0x3000)
    } else {
        Bambu_StartStreamEx(handle, 0)
    }
}

#[no_mangle]
pub unsafe extern "C" fn Bambu_SendMessage(
    handle: *mut LocalTunnel,
    _ctrl: i32,
    _data: *const u8,
    _data_len: i32,
) -> c_int {
    if handle.is_null() {
        return -1;
    }

    /* TODO: Used for the SD card explorer but "not available on LAN mode" (probably accesible still) */

    -1
}

#[no_mangle]
pub unsafe extern "C" fn Bambu_RecvMessage(
    handle: *mut LocalTunnel,
    _ctrl: *mut i32,
    _data: *mut u8,
    _data_len: *mut i32,
) -> c_int {
    if handle.is_null() {
        return -1;
    }

    /* TODO: Used for the SD card explorer but "not available on LAN mode" (probably accesible still) */

    -1
}

#[no_mangle]
pub unsafe extern "C" fn Bambu_ReadSample(
    handle: *mut LocalTunnel,
    sample: *mut BambuSample,
) -> c_int {
    if handle.is_null() {
        return -1;
    }

    let tunnel = unsafe { &mut *handle };

    if let Err(e) = tunnel.read_sample(&mut *sample) {
        if let Some(io_error) = e.downcast_ref::<io::Error>() {
            if io_error.kind() == io::ErrorKind::WouldBlock
                || io_error.kind() == io::ErrorKind::Interrupted
            {
                return BAMBU_WOULD_BLOCK_ERROR;
            }
        }

        eprintln!("TURION: {e:?}");

        return -1;
    }

    0
}

#[no_mangle]
pub unsafe extern "C" fn Bambu_SetLogger(
    _handle: *mut LocalTunnel,
    _logger: *const c_void,
    _ctx: *const c_void,
) {
    // no op
}

#[no_mangle]
pub unsafe extern "C" fn Bambu_Init() {
    // no op
}

#[no_mangle]
pub unsafe extern "C" fn Bambu_Deinit() {
    // no op
}

#[no_mangle]
pub unsafe extern "C" fn Bambu_GetLastErrorMsg() -> *mut c_char {
    core::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn Bambu_GetDuration(_handle: *mut LocalTunnel) -> c_ulong {
    // no op
    c_ulong::MAX
}

#[no_mangle]
pub unsafe extern "C" fn Bambu_FreeLogMsg(_msg: *const c_char) {
    // no op
}
