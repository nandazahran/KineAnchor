// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.nandazahran.kineanchor

/**
 * JNI bridge to `kineanchor-android-ffi` (Rust). Zero-allocation C ABI; packed returns stay as
 * defined by the Rust side and are unpacked with [unpackFirstFloat] / [unpackSecondFloat].
 *
 * [available] is false when the native library could not be loaded. Callers must check it and
 * surface [loadError] instead of calling into a missing library, which would throw on first use.
 */
object NativeEngine {
    /** Human-readable reason the native library is unavailable; null when it loaded. */
    val loadError: String? = runCatching {
        System.loadLibrary("kineanchor_android_ffi")
    }.exceptionOrNull()?.let { error ->
        "${error::class.java.simpleName}: ${error.message ?: "unknown load failure"}"
    }

    val available: Boolean = loadError == null

    @JvmStatic
    external fun nativeCreate(): Long

    @JvmStatic
    external fun nativeDestroy(ptr: Long)

    @JvmStatic
    external fun nativeZero(ptr: Long)

    @JvmStatic
    external fun nativeSetConfig(
        ptr: Long,
        vehicle: Int,
        source: Int,
        density: Float,
        dotRadiusPx: Float,
        opacity: Float,
        gain: Float,
        syntheticAmplitude: Float,
    )

    @JvmStatic
    external fun nativePushSample(
        ptr: Long,
        t: Double,
        ax: Float,
        ay: Float,
        az: Float,
        gx: Float,
        gy: Float,
        gz: Float,
        hasGravity: Boolean,
        gvx: Float,
        gvy: Float,
        gvz: Float,
    )

    @JvmStatic
    external fun nativeTick(
        ptr: Long,
        now: Double,
        width: Float,
        height: Float,
    ): Int

    @JvmStatic
    external fun nativeGetDotXY(
        ptr: Long,
        index: Int,
    ): Long

    @JvmStatic
    external fun nativeGetDotRA(
        ptr: Long,
        index: Int,
    ): Long

    @JvmStatic
    external fun nativeGetStatus(
        ptr: Long,
        now: Double,
    ): Int

    fun unpackFirstFloat(packed: Long): Float = Float.fromBits((packed ushr 32).toInt())

    fun unpackSecondFloat(packed: Long): Float = Float.fromBits((packed and 0xFFFFFFFFL).toInt())
}
