// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.nandazahran.kineanchor

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.graphics.Canvas
import android.graphics.Paint
import android.hardware.Sensor
import android.hardware.SensorEvent
import android.hardware.SensorEventListener
import android.hardware.SensorManager
import android.hardware.input.InputManager
import android.os.Build
import android.os.IBinder
import android.os.SystemClock
import android.provider.Settings
import android.view.Choreographer
import android.view.Gravity
import android.view.View
import android.view.WindowManager

class OverlayService : Service(), SensorEventListener, Choreographer.FrameCallback {
    data class StateSnapshot(
        val running: Boolean = false,
        val paused: Boolean = false,
        val source: Int = 0,
        val vehicle: Int = 0,
        val statusCode: Int = 0,
        val horizonX: Float = 0f,
        val horizonY: Float = 0f,
        val hasAccel: Boolean = true,
        val hasGyro: Boolean = true,
        val windowError: String? = null,
    )

    companion object {
        const val ACTION_START = "io.github.nandazahran.kineanchor.OVERLAY_START"
        const val ACTION_STOP = "io.github.nandazahran.kineanchor.OVERLAY_STOP"
        const val ACTION_ZERO = "io.github.nandazahran.kineanchor.OVERLAY_ZERO"
        const val ACTION_PAUSE = "io.github.nandazahran.kineanchor.OVERLAY_PAUSE"
        const val ACTION_RESUME = "io.github.nandazahran.kineanchor.OVERLAY_RESUME"
        const val ACTION_UPDATE_CONFIG = "io.github.nandazahran.kineanchor.OVERLAY_CONFIG"

        const val EXTRA_VEHICLE = "vehicle" // 0 = Car, 1 = Ship
        const val EXTRA_SOURCE = "source" // 0 = Sensor, 1 = Synthetic
        const val EXTRA_DENSITY = "density"
        const val EXTRA_RADIUS = "radius"
        const val EXTRA_OPACITY = "opacity"
        const val EXTRA_GAIN = "gain"
        const val EXTRA_SYNTH_AMP = "synth_amp"

        private const val CHANNEL_ID = "kineanchor_overlay"
        private const val NOTIF_ID = 47475

        @Volatile
        var currentState: StateSnapshot = StateSnapshot()
            private set
    }

    private var enginePtr: Long = 0L
    private var windowManager: WindowManager? = null
    private var overlayView: DotOverlayView? = null
    private var sensorManager: SensorManager? = null
    private var running = false
    private var paused = false
    private var configuredVehicle = 0
    private var configuredSource = 0

    @Volatile private var latestGyro = FloatArray(3)
    @Volatile private var latestGravity: FloatArray? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(
        intent: Intent?,
        flags: Int,
        startId: Int,
    ): Int {
        when (intent?.action) {
            ACTION_STOP -> {
                stopOverlay()
                stopSelf()
                return START_NOT_STICKY
            }
            ACTION_ZERO -> {
                if (enginePtr != 0L) NativeEngine.nativeZero(enginePtr)
                return START_STICKY
            }
            ACTION_PAUSE -> {
                paused = true
                overlayView?.invalidate()
                publishState(windowError = null)
                return START_STICKY
            }
            ACTION_RESUME -> {
                paused = false
                overlayView?.invalidate()
                publishState(windowError = null)
                return START_STICKY
            }
            ACTION_UPDATE_CONFIG -> {
                if (enginePtr != 0L) {
                    applyIntentConfig(intent)
                    publishState(windowError = null)
                }
                return START_STICKY
            }
        }

        // A restart with a null intent has no configuration to restore; stop instead of
        // rendering an overlay with default settings the user never chose.
        if (intent == null && !running) {
            stopSelf()
            return START_NOT_STICKY
        }

        if (!NativeEngine.available) {
            val err = "Native engine failed: ${NativeEngine.loadError}"
            startForegroundNotification(err)
            publishState(windowError = err)
            stopSelf()
            return START_NOT_STICKY
        }

        if (!Settings.canDrawOverlays(this)) {
            publishState(windowError = "Overlay permission missing or revoked.")
            stopSelf()
            return START_NOT_STICKY
        }

        startForegroundNotification(null)
        if (!running) {
            // startOverlay reports its own failure and stops the service; never claim Live.
            startOverlay(intent)
        } else if (intent != null) {
            paused = false
            applyIntentConfig(intent)
            publishState(windowError = null)
        }
        return if (running) START_STICKY else START_NOT_STICKY
    }

    override fun onDestroy() {
        stopOverlay()
        super.onDestroy()
    }

    private fun applyIntentConfig(intent: Intent) {
        val vehicle = intent.getIntExtra(EXTRA_VEHICLE, 0)
        val source = intent.getIntExtra(EXTRA_SOURCE, 0)
        val density = intent.getFloatExtra(EXTRA_DENSITY, 1.0f)
        val radius = intent.getFloatExtra(EXTRA_RADIUS, 10.0f)
        val opacity = intent.getFloatExtra(EXTRA_OPACITY, 0.75f).coerceAtMost(0.8f)
        val gain = intent.getFloatExtra(EXTRA_GAIN, 1.0f)
        val synthAmp = intent.getFloatExtra(EXTRA_SYNTH_AMP, 0.25f)
        configuredVehicle = vehicle
        configuredSource = source
        NativeEngine.nativeSetConfig(enginePtr, vehicle, source, density, radius, opacity, gain, synthAmp)
    }

    private fun startOverlay(intent: Intent?) {
        if (!NativeEngine.available) {
            publishState(windowError = "Native engine failed: ${NativeEngine.loadError}")
            stopSelf()
            return
        }
        enginePtr = runCatching { NativeEngine.nativeCreate() }.getOrDefault(0L)
        if (enginePtr == 0L) {
            publishState(windowError = "Could not initialize native motion engine.")
            stopSelf()
            return
        }
        if (intent != null) applyIntentConfig(intent)

        val wm = getSystemService(Context.WINDOW_SERVICE) as WindowManager
        windowManager = wm

        // Android 12+ untrusted-touch rule (R12): window alpha MUST be <= maximum obscuring opacity (0.8f)
        val maxTouchAlpha = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            val im = getSystemService(Context.INPUT_SERVICE) as InputManager
            im.maximumObscuringOpacityForTouch.coerceAtMost(0.8f)
        } else {
            0.8f
        }

        val params = WindowManager.LayoutParams(
            WindowManager.LayoutParams.MATCH_PARENT,
            WindowManager.LayoutParams.MATCH_PARENT,
            WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
            WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE or
                WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
                WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN or
                WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS,
            android.graphics.PixelFormat.TRANSLUCENT,
        ).apply {
            gravity = Gravity.TOP or Gravity.START
            alpha = maxTouchAlpha
        }

        val view = DotOverlayView(this)
        overlayView = view
        val addResult = runCatching { wm.addView(view, params) }
        if (addResult.isFailure) {
            // Adding the overlay window can fail (permission revoked mid-flight, bad token).
            overlayView = null
            stopOverlay(preserveWindowError = true)
            publishState(
                windowError = addResult.exceptionOrNull()?.message
                    ?: "Android rejected overlay window. Check display-over-apps permission.",
            )
            stopSelf()
            return
        }

        val sm = getSystemService(Context.SENSOR_SERVICE) as SensorManager
        sensorManager = sm
        val accelSensor = sm.getDefaultSensor(Sensor.TYPE_LINEAR_ACCELERATION)
        val gyroSensor = sm.getDefaultSensor(Sensor.TYPE_GYROSCOPE)
        val gravSensor = sm.getDefaultSensor(Sensor.TYPE_GRAVITY)
        accelSensor?.let { sm.registerListener(this, it, SensorManager.SENSOR_DELAY_GAME) }
        gyroSensor?.let { sm.registerListener(this, it, SensorManager.SENSOR_DELAY_GAME) }
        gravSensor?.let { sm.registerListener(this, it, SensorManager.SENSOR_DELAY_GAME) }

        running = true
        paused = false
        currentState = StateSnapshot(
            running = true,
            paused = false,
            source = configuredSource,
            vehicle = configuredVehicle,
            hasAccel = accelSensor != null,
            hasGyro = gyroSensor != null,
            windowError = null,
        )
        Choreographer.getInstance().postFrameCallback(this)
    }

    private fun stopOverlay(preserveWindowError: Boolean = false) {
        running = false
        paused = false
        Choreographer.getInstance().removeFrameCallback(this)
        sensorManager?.unregisterListener(this)
        sensorManager = null
        overlayView?.let { v ->
            runCatching { windowManager?.removeView(v) }
        }
        overlayView = null
        if (enginePtr != 0L) {
            NativeEngine.nativeDestroy(enginePtr)
            enginePtr = 0L
        }
        currentState = StateSnapshot(
            running = false,
            paused = false,
            windowError = if (preserveWindowError) currentState.windowError else null,
        )
    }

    private fun publishState(
        windowError: String?,
        horizonX: Float = currentState.horizonX,
        horizonY: Float = currentState.horizonY,
        statusCode: Int = currentState.statusCode,
    ) {
        currentState = currentState.copy(
            running = running,
            paused = paused,
            source = configuredSource,
            vehicle = configuredVehicle,
            statusCode = statusCode,
            horizonX = horizonX,
            horizonY = horizonY,
            windowError = windowError,
        )
    }

    override fun doFrame(frameTimeNanos: Long) {
        if (!running || enginePtr == 0L) return
        overlayView?.invalidate()
        Choreographer.getInstance().postFrameCallback(this)
    }

    override fun onSensorChanged(event: SensorEvent) {
        if (!running || paused || enginePtr == 0L) return
        when (event.sensor.type) {
            Sensor.TYPE_GYROSCOPE -> {
                latestGyro[0] = event.values[0]
                latestGyro[1] = event.values[1]
                latestGyro[2] = event.values[2]
            }
            Sensor.TYPE_GRAVITY -> {
                latestGravity = floatArrayOf(event.values[0], event.values[1], event.values[2])
            }
            Sensor.TYPE_LINEAR_ACCELERATION -> {
                val tSec = event.timestamp / 1_000_000_000.0
                val g = latestGravity
                NativeEngine.nativePushSample(
                    enginePtr,
                    tSec,
                    event.values[0],
                    event.values[1],
                    event.values[2],
                    latestGyro[0],
                    latestGyro[1],
                    latestGyro[2],
                    g != null,
                    g?.get(0) ?: 0f,
                    g?.get(1) ?: 0f,
                    g?.get(2) ?: 0f,
                )
            }
        }
    }

    override fun onAccuracyChanged(
        sensor: Sensor?,
        accuracy: Int,
    ) = Unit

    private fun startForegroundNotification(failureText: String?) {
        val nm = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "KineAnchor Overlay",
                NotificationManager.IMPORTANCE_LOW,
            )
            nm.createNotificationChannel(channel)
        }
        val title = if (failureText == null) {
            "KineAnchor overlay active"
        } else {
            "KineAnchor overlay unavailable"
        }
        val body = failureText ?: "Drawing peripheral motion cues over screen"
        val notif = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            Notification.Builder(this, CHANNEL_ID)
                .setContentTitle(title)
                .setContentText(body)
                .setSmallIcon(android.R.drawable.ic_menu_compass)
                .build()
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(this)
                .setContentTitle(title)
                .setContentText(body)
                .setSmallIcon(android.R.drawable.ic_menu_compass)
                .build()
        }

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startForeground(NOTIF_ID, notif, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        } else {
            startForeground(NOTIF_ID, notif)
        }
    }

    private inner class DotOverlayView(context: Context) : View(context) {
        private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            style = Paint.Style.FILL
            color = 0xFFDCFAFF.toInt()
        }

        override fun onDraw(canvas: Canvas) {
            super.onDraw(canvas)
            val ptr = enginePtr
            if (ptr == 0L || width <= 0 || height <= 0 || paused) return

            val nowSec = SystemClock.elapsedRealtimeNanos() / 1_000_000_000.0
            val count = NativeEngine.nativeTick(ptr, nowSec, width.toFloat(), height.toFloat())
            val statusCode = NativeEngine.nativeGetStatus(ptr)
            var sumDx = 0f
            var sumDy = 0f
            val cx = width * 0.5f
            val cy = height * 0.5f
            for (i in 0 until count) {
                val xy = NativeEngine.nativeGetDotXY(ptr, i)
                val ra = NativeEngine.nativeGetDotRA(ptr, i)
                val x = NativeEngine.unpackFirstFloat(xy)
                val y = NativeEngine.unpackSecondFloat(xy)
                val r = NativeEngine.unpackFirstFloat(ra)
                val alpha = NativeEngine.unpackSecondFloat(ra)
                sumDx += (x - cx)
                sumDy += (y - cy)
                paint.alpha = (alpha.coerceIn(0f, 1f) * 255f).toInt()
                canvas.drawCircle(x, y, r, paint)
            }
            val normX = if (count > 0) (sumDx / count) / (width.coerceAtLeast(1) * 0.25f) else 0f
            val normY = if (count > 0) (sumDy / count) / (height.coerceAtLeast(1) * 0.25f) else 0f
            publishState(
                windowError = null,
                horizonX = normX.coerceIn(-1f, 1f),
                horizonY = normY.coerceIn(-1f, 1f),
                statusCode = statusCode,
            )
        }
    }
}
