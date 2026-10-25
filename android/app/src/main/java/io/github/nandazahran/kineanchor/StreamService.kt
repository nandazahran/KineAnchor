// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.nandazahran.kineanchor

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.hardware.Sensor
import android.hardware.SensorEvent
import android.hardware.SensorEventListener
import android.hardware.SensorManager
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import android.os.SystemClock
import java.io.BufferedReader
import java.io.InputStreamReader
import java.io.OutputStreamWriter
import java.net.InetSocketAddress
import java.net.ServerSocket
import java.net.Socket
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean

class StreamService : Service(), SensorEventListener {
    private val running = AtomicBoolean(false)
    private val sampleQueue = LinkedBlockingQueue<Protocol.Message.Sample>(8)
    private var sensorManager: SensorManager? = null
    private var wakeLock: PowerManager.WakeLock? = null
    private var workerThread: Thread? = null

    @Volatile private var latestGyro = FloatArray(3)
    @Volatile private var latestGravity: FloatArray? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) {
            stopSelf()
            return START_NOT_STICKY
        }

        val targetHost = intent?.getStringExtra(EXTRA_HOST)
        val targetPort = intent?.getIntExtra(EXTRA_PORT, Protocol.DEFAULT_PORT) ?: Protocol.DEFAULT_PORT
        val pairSecret = intent?.getStringExtra(EXTRA_PAIR_SECRET)

        startForegroundNotification()
        acquireWakeLock()
        registerSensors()

        if (running.compareAndSet(false, true)) {
            workerThread = Thread {
                runNetworkLoop(targetHost, targetPort, pairSecret)
            }.apply {
                name = "kineanchor-stream"
                start()
            }
        }
        return START_STICKY
    }

    override fun onDestroy() {
        running.set(false)
        sensorManager?.unregisterListener(this)
        wakeLock?.let { if (it.isHeld) it.release() }
        super.onDestroy()
    }

    private fun startForegroundNotification() {
        val channelId = "kineanchor_stream"
        val nm = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        val channel = NotificationChannel(
            channelId,
            "KineAnchor Sensor Stream",
            NotificationManager.IMPORTANCE_LOW,
        )
        nm.createNotificationChannel(channel)
        val notif = Notification.Builder(this, channelId)
            .setContentTitle("KineAnchor Streaming to PC")
            .setContentText("50Hz vehicle IMU stream active")
            .setSmallIcon(android.R.drawable.ic_menu_compass)
            .build()

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startForeground(1, notif, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        } else {
            startForeground(1, notif)
        }
    }

    private fun acquireWakeLock() {
        val pm = getSystemService(Context.POWER_SERVICE) as PowerManager
        wakeLock = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "KineAnchor::StreamWakeLock").apply {
            acquire(6 * 60 * 60 * 1000L)
        }
    }

    private fun registerSensors() {
        val sm = getSystemService(Context.SENSOR_SERVICE) as SensorManager
        sensorManager = sm
        sm.getDefaultSensor(Sensor.TYPE_LINEAR_ACCELERATION)?.let {
            sm.registerListener(this, it, SensorManager.SENSOR_DELAY_GAME)
        }
        sm.getDefaultSensor(Sensor.TYPE_GYROSCOPE)?.let {
            sm.registerListener(this, it, SensorManager.SENSOR_DELAY_GAME)
        }
        sm.getDefaultSensor(Sensor.TYPE_GRAVITY)?.let {
            sm.registerListener(this, it, SensorManager.SENSOR_DELAY_GAME)
        }
    }

    override fun onSensorChanged(event: SensorEvent) {
        when (event.sensor.type) {
            Sensor.TYPE_GYROSCOPE -> {
                latestGyro = floatArrayOf(event.values[0], event.values[1], event.values[2])
            }
            Sensor.TYPE_GRAVITY -> {
                latestGravity = floatArrayOf(event.values[0], event.values[1], event.values[2])
            }
            Sensor.TYPE_LINEAR_ACCELERATION -> {
                val t = SystemClock.elapsedRealtimeNanos() / 1e9
                val g = latestGyro
                val grav = latestGravity
                val sample = Protocol.Message.Sample(
                    t = t,
                    ax = event.values[0],
                    ay = event.values[1],
                    az = event.values[2],
                    gx = g[0],
                    gy = g[1],
                    gz = g[2],
                    gvx = grav?.get(0),
                    gvy = grav?.get(1),
                    gvz = grav?.get(2),
                )
                sampleQueue.poll()
                sampleQueue.offer(sample)
            }
        }
    }

    override fun onAccuracyChanged(sensor: Sensor?, accuracy: Int) {}

    private fun runNetworkLoop(targetHost: String?, port: Int, initialSecret: String?) {
        val prefs = getSharedPreferences("kineanchor_prefs", Context.MODE_PRIVATE)
        var currentSecret = initialSecret
        val server = runCatching {
            ServerSocket(port).apply { soTimeout = 500 }
        }.getOrNull()

        try {
            while (running.get()) {
                var sock: Socket? = null
                // 1. Accept incoming connection from desktop (Q2 primary: phone listens, desktop dials)
                if (server != null) {
                    sock = runCatching { server.accept() }.getOrNull()
                }
                // 2. Or dial desktop host if known from QR (LAN fallback)
                if (sock == null && !targetHost.isNullOrEmpty()) {
                    sock = runCatching {
                        Socket().apply {
                            connect(InetSocketAddress(targetHost, port), 400)
                        }
                    }.getOrNull()
                }

                if (sock != null) {
                    sock.use { activeSocket ->
                        activeSocket.tcpNoDelay = true
                        activeSocket.soTimeout = 1500
                        val savedToken = prefs.getString("paired_token", null)
                        val hello = Protocol.Message.Hello(
                            device = Build.MODEL ?: "Android",
                            token = if (currentSecret == null) savedToken else null,
                            pair = currentSecret,
                        )
                        val writer = OutputStreamWriter(activeSocket.getOutputStream(), Charsets.UTF_8)
                        val reader = BufferedReader(InputStreamReader(activeSocket.getInputStream(), Charsets.UTF_8))

                        writer.write(Protocol.encode(hello))
                        writer.flush()

                        val welcomeLine = reader.readLine() ?: return@use
                        when (val reply = Protocol.decode(welcomeLine)) {
                            is Protocol.Message.Welcome -> {
                                prefs.edit()
                                    .putString("paired_token", reply.token)
                                    .putString("paired_desktop", reply.name)
                                    .apply()
                                currentSecret = null
                                streamSamples(writer)
                            }
                            else -> return@use
                        }
                    }
                } else {
                    Thread.sleep(250)
                }
            }
        } finally {
            runCatching { server?.close() }
        }
    }

    private fun streamSamples(writer: OutputStreamWriter) {
        while (running.get()) {
            val sample = sampleQueue.poll(500, TimeUnit.MILLISECONDS)
            val msg = sample ?: Protocol.Message.Ping
            writer.write(Protocol.encode(msg))
            writer.flush()
        }
    }

    companion object {
        const val ACTION_START = "io.github.nandazahran.kineanchor.START"
        const val ACTION_STOP = "io.github.nandazahran.kineanchor.STOP"
        const val EXTRA_HOST = "host"
        const val EXTRA_PORT = "port"
        const val EXTRA_PAIR_SECRET = "pair_secret"
    }
}
