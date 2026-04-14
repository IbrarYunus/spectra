use crate::audio::{AudioBuffer, AudioSource, Keepalive};
use anyhow::{anyhow, Result};
use std::ffi::c_void;
use std::sync::Arc;

type SpectraAudioCallback =
    unsafe extern "C" fn(*const f32, i32, i32, *mut c_void);

extern "C" {
    fn spectra_sc_start(
        cb: SpectraAudioCallback,
        ctx: *mut c_void,
        sr_out: *mut i32,
        err_out: *mut i32,
    ) -> i32;
    fn spectra_sc_stop();
}

unsafe extern "C" fn on_audio(
    data: *const f32,
    frames: i32,
    channels: i32,
    ctx: *mut c_void,
) {
    if data.is_null() || frames <= 0 || ctx.is_null() {
        return;
    }
    let arc = &*(ctx as *const Arc<AudioBuffer>);
    let n = frames as usize;
    if channels <= 1 {
        let slice = std::slice::from_raw_parts(data, n);
        arc.push(slice);
    } else {
        let slice = std::slice::from_raw_parts(data, n * channels as usize);
        let mono: Vec<f32> = slice
            .chunks(channels as usize)
            .map(|c| c.iter().sum::<f32>() / channels as f32)
            .collect();
        arc.push(&mono);
    }
}

pub struct ScGuard {
    ctx: *mut Arc<AudioBuffer>,
}

impl Drop for ScGuard {
    fn drop(&mut self) {
        unsafe {
            spectra_sc_stop();
            if !self.ctx.is_null() {
                let _ = Box::from_raw(self.ctx);
            }
        }
    }
}

unsafe impl Send for ScGuard {}
unsafe impl Sync for ScGuard {}

pub fn start_screen_capture() -> Result<AudioSource> {
    let buffer = AudioBuffer::new(48000 * 4);
    let ctx_box: Box<Arc<AudioBuffer>> = Box::new(buffer.clone());
    let ctx_ptr = Box::into_raw(ctx_box);

    let mut sr: i32 = 0;
    let mut err: i32 = 0;
    let ret = unsafe {
        spectra_sc_start(on_audio, ctx_ptr as *mut c_void, &mut sr, &mut err)
    };

    if ret != 0 {
        unsafe { let _ = Box::from_raw(ctx_ptr); }
        let hint = "Make sure your terminal has Screen Recording permission: \
                    System Settings → Privacy & Security → Screen Recording \
                    (add your terminal app, e.g. Hyper / iTerm / Terminal), \
                    then fully quit and reopen the terminal and try again.";
        return Err(anyhow!(
            "ScreenCaptureKit failed to start (rc={ret}, err={err}). {hint}"
        ));
    }

    let sample_rate = if sr > 0 { sr as u32 } else { 48000 };
    let guard = ScGuard { ctx: ctx_ptr };

    Ok(AudioSource {
        sample_rate,
        buffer,
        source_label: "system audio (ScreenCaptureKit)".into(),
        _keepalive: Keepalive::System(Box::new(guard)),
    })
}
