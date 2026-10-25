// SPDX-License-Identifier: GPL-3.0-or-later

//! Zero-dependency C-ABI JNI bridge exposing `kineanchor-core::Engine` to `io.github.nandazahran.kineanchor.NativeEngine`.

use core::ffi::c_void;
use kineanchor_core::{Config, Engine, MotionSample, Source, Status, Vehicle};

pub type JniEnv = *mut c_void;
pub type JClass = *mut c_void;
pub type JLong = i64;
pub type JInt = i32;
pub type JFloat = f32;
pub type JDouble = f64;
pub type JBoolean = u8;

#[inline]
fn pack_f32_pair(high: f32, low: f32) -> JLong {
    let hi = (high.to_bits() as u64) << 32;
    let lo = low.to_bits() as u64;
    (hi | lo) as JLong
}

#[inline]
pub fn unpack_f32_pair(packed: JLong) -> (f32, f32) {
    let bits = packed as u64;
    let hi = f32::from_bits((bits >> 32) as u32);
    let lo = f32::from_bits((bits & 0xFFFF_FFFF) as u32);
    (hi, lo)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_nandazahran_kineanchor_NativeEngine_nativeCreate(
    _env: JniEnv,
    _class: JClass,
) -> JLong {
    let engine = Box::new(Engine::new(Config::default()));
    Box::into_raw(engine) as usize as JLong
}

/// # Safety
/// `ptr` must be a valid pointer returned by `nativeCreate` and not yet destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_nandazahran_kineanchor_NativeEngine_nativeDestroy(
    _env: JniEnv,
    _class: JClass,
    ptr: JLong,
) {
    if ptr != 0 {
        let _ = unsafe { Box::from_raw(ptr as usize as *mut Engine) };
    }
}

/// # Safety
/// `ptr` must be a valid pointer returned by `nativeCreate`.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_nandazahran_kineanchor_NativeEngine_nativeZero(
    _env: JniEnv,
    _class: JClass,
    ptr: JLong,
) {
    if ptr == 0 {
        return;
    }
    let engine = unsafe { &mut *(ptr as usize as *mut Engine) };
    engine.zero();
}

/// # Safety
/// `ptr` must be a valid pointer returned by `nativeCreate`.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_nandazahran_kineanchor_NativeEngine_nativeSetConfig(
    _env: JniEnv,
    _class: JClass,
    ptr: JLong,
    vehicle: JInt,
    source: JInt,
    density: JFloat,
    dot_radius_px: JFloat,
    opacity: JFloat,
    gain: JFloat,
    synthetic_amplitude: JFloat,
) {
    if ptr == 0 {
        return;
    }
    let engine = unsafe { &mut *(ptr as usize as *mut Engine) };
    let mut cfg = *engine.config();
    cfg.vehicle = if vehicle == 1 {
        Vehicle::Ship
    } else {
        Vehicle::Car
    };
    cfg.source = if source == 1 {
        Source::Synthetic
    } else {
        Source::Sensor
    };
    cfg.density = density.clamp(0.1, 3.0);
    cfg.dot_radius_px = dot_radius_px.clamp(2.0, 24.0);
    cfg.opacity = opacity.clamp(0.05, 0.8);
    cfg.gain = gain.clamp(0.1, 3.0);
    cfg.synthetic.amplitude = synthetic_amplitude.clamp(0.0, 1.0);
    engine.set_config(cfg);
}

/// # Safety
/// `ptr` must be a valid pointer returned by `nativeCreate`.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_nandazahran_kineanchor_NativeEngine_nativePushSample(
    _env: JniEnv,
    _class: JClass,
    ptr: JLong,
    t: JDouble,
    ax: JFloat,
    ay: JFloat,
    az: JFloat,
    gx: JFloat,
    gy: JFloat,
    gz: JFloat,
    has_gravity: JBoolean,
    gvx: JFloat,
    gvy: JFloat,
    gvz: JFloat,
) {
    if ptr == 0 {
        return;
    }
    let engine = unsafe { &mut *(ptr as usize as *mut Engine) };
    let gravity = if has_gravity != 0 {
        Some([gvx, gvy, gvz])
    } else {
        None
    };
    engine.push(
        MotionSample {
            t,
            ax,
            ay,
            az,
            gx,
            gy,
            gz,
            gravity,
        },
        t,
    );
}

/// Steps the engine for `now` (seconds) and viewport `(width, height)`, returning the number of dots.
///
/// # Safety
/// `ptr` must be a valid pointer returned by `nativeCreate`.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_nandazahran_kineanchor_NativeEngine_nativeTick(
    _env: JniEnv,
    _class: JClass,
    ptr: JLong,
    now: JDouble,
    width: JFloat,
    height: JFloat,
) -> JInt {
    if ptr == 0 {
        return 0;
    }
    let engine = unsafe { &mut *(ptr as usize as *mut Engine) };
    let (dots, _status) = engine.tick(now, [width.max(1.0), height.max(1.0)]);
    dots.len() as JInt
}

/// Returns packed `(x, y)` f32 coordinates for dot `index` after `nativeTick`.
///
/// # Safety
/// `ptr` must be a valid pointer returned by `nativeCreate`.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_nandazahran_kineanchor_NativeEngine_nativeGetDotXY(
    _env: JniEnv,
    _class: JClass,
    ptr: JLong,
    index: JInt,
) -> JLong {
    if ptr == 0 || index < 0 {
        return 0;
    }
    let engine = unsafe { &*(ptr as usize as *const Engine) };
    let dots = engine.current_dots();
    let Some(dot) = dots.get(index as usize) else {
        return 0;
    };
    pack_f32_pair(dot.x, dot.y)
}

/// Returns packed `(r, alpha)` f32 values for dot `index` after `nativeTick`.
///
/// # Safety
/// `ptr` must be a valid pointer returned by `nativeCreate`.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_nandazahran_kineanchor_NativeEngine_nativeGetDotRA(
    _env: JniEnv,
    _class: JClass,
    ptr: JLong,
    index: JInt,
) -> JLong {
    if ptr == 0 || index < 0 {
        return 0;
    }
    let engine = unsafe { &*(ptr as usize as *const Engine) };
    let dots = engine.current_dots();
    let Some(dot) = dots.get(index as usize) else {
        return 0;
    };
    pack_f32_pair(dot.r, dot.alpha)
}

/// Returns status code: 0 = Live, 1 = Stale, 2 = Synthetic, 3 = Calibrating.
///
/// # Safety
/// `ptr` must be a valid pointer returned by `nativeCreate`.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_nandazahran_kineanchor_NativeEngine_nativeGetStatus(
    _env: JniEnv,
    _class: JClass,
    ptr: JLong,
    now: JDouble,
) -> JInt {
    if ptr == 0 {
        return 1;
    }
    let engine = unsafe { &*(ptr as usize as *const Engine) };
    match engine.status(now) {
        Status::Live { .. } => 0,
        Status::Stale { .. } => 1,
        Status::Synthetic => 2,
        Status::Calibrating => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::ptr::null_mut;

    #[test]
    fn test_jni_lifecycle_and_tick() {
        let env = null_mut();
        let cls = null_mut();
        let ptr = Java_io_github_nandazahran_kineanchor_NativeEngine_nativeCreate(env, cls);
        assert_ne!(ptr, 0);

        unsafe {
            Java_io_github_nandazahran_kineanchor_NativeEngine_nativeSetConfig(
                env, cls, ptr, 0, // Car
                0, // Sensor
                1.0, 6.0, 0.75, 1.0, 0.25,
            );
            Java_io_github_nandazahran_kineanchor_NativeEngine_nativePushSample(
                env, cls, ptr, 1.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.1, 1, 0.0, 0.0, 9.81,
            );
            let count = Java_io_github_nandazahran_kineanchor_NativeEngine_nativeTick(
                env, cls, ptr, 1.02, 1080.0, 2400.0,
            );
            assert!(count > 0);

            let xy =
                Java_io_github_nandazahran_kineanchor_NativeEngine_nativeGetDotXY(env, cls, ptr, 0);
            let (x, y) = unpack_f32_pair(xy);
            assert!((0.0..=1080.0).contains(&x));
            assert!((0.0..=2400.0).contains(&y));

            let ra =
                Java_io_github_nandazahran_kineanchor_NativeEngine_nativeGetDotRA(env, cls, ptr, 0);
            let (r, alpha) = unpack_f32_pair(ra);
            assert!((r - 6.0).abs() < 1e-4);
            assert!(alpha > 0.0 && alpha <= 0.8);

            let status = Java_io_github_nandazahran_kineanchor_NativeEngine_nativeGetStatus(
                env, cls, ptr, 1.05,
            );
            assert_eq!(status, 0); // Live

            Java_io_github_nandazahran_kineanchor_NativeEngine_nativeZero(env, cls, ptr);
            Java_io_github_nandazahran_kineanchor_NativeEngine_nativeDestroy(env, cls, ptr);
        }
    }
}
