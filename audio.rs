//
// sprocketnes/audio.rs
//
// Author: Patrick Walton
//

// TODO: This module is very unsafe. Adding a reader-writer audio lock to SDL would help make it
// safe.

use libc::{c_int, c_void, uint8_t};
use sdl2::audio::ll::SDL_AudioSpec;
use sdl2::audio::{AudioDevice, AudioS16LSB};
use std::cast;
use std::cmp;
use std::ptr;
use std::raw::Slice;
use std::unstable::mutex::{NATIVE_MUTEX_INIT, StaticNativeMutex};

//
// The audio callback
//

static SAMPLE_COUNT: uint = 4410 * 2;

static mut g_audio_device: Option<AudioDevice> = None;

static mut g_output_buffer: Option<*mut OutputBuffer> = None;

pub static mut g_mutex: StaticNativeMutex = NATIVE_MUTEX_INIT;

pub struct OutputBuffer {
    pub samples: [uint8_t, ..SAMPLE_COUNT],
    pub play_offset: uint,
}

extern "C" fn nes_audio_callback(_: *c_void, stream: *uint8_t, len: c_int) {
    unsafe {
        let samples: &mut [uint8_t] = cast::transmute(Slice {
            data: stream,
            len: len as uint,
        });

        let output_buffer: &mut OutputBuffer = cast::transmute(g_output_buffer.unwrap());
        let play_offset = output_buffer.play_offset;
        let output_buffer_len = output_buffer.samples.len();

        for i in range(0, samples.len()) {
            if i + play_offset >= output_buffer_len {
                break;
            }
            samples[i] = output_buffer.samples[i + play_offset];
        }

        let lock = g_mutex.lock();
        output_buffer.play_offset = cmp::min(play_offset + samples.len(), output_buffer_len);
        lock.signal();
    }
}

//
// Audio initialization
//

pub fn open() -> *mut OutputBuffer {
    let output_buffer = box OutputBuffer {
        samples: [ 0, ..8820 ],
        play_offset: 0,
    };
    let output_buffer_ptr: *mut OutputBuffer = unsafe {
        cast::transmute(&*output_buffer)
    };

    unsafe {
        g_output_buffer = Some(output_buffer_ptr)
    }

    let spec = SDL_AudioSpec {
        freq: 44100,
        format: AudioS16LSB,
        channels: 1,
        silence: 0,
        samples: 4410,
        padding: 0,
        size: 0,
        userdata: ptr::null(),
        callback: Some(nes_audio_callback),
    };

    let (audio_device, _) = unsafe {
        AudioDevice::open(None, 0, cast::transmute(&spec)).unwrap()
    };
    audio_device.resume();

    unsafe {
        g_audio_device = Some(audio_device);
        cast::forget(output_buffer);
    }

    output_buffer_ptr
}

//
// Audio tear-down
//

pub fn close() {
    unsafe {
        match g_audio_device {
            None => {}
            Some(audio_device) => {
                audio_device.close();
                g_audio_device = None
            }
        }
    }
}

pub struct AudioLock;

impl Drop for AudioLock {
    fn drop(&mut self) {
        unsafe {
            match g_audio_device {
                None => {}
                Some(audio_device) => audio_device.unlock(),
            }
        }
    }
}

impl AudioLock {
    pub fn lock() -> AudioLock {
        unsafe {
            match g_audio_device {
                None => {}
                Some(audio_device) => audio_device.lock(),
            }
        }
        AudioLock
    }
}

