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

    /** The socket currently serving a session, so a restart can close it and unblock the loop. */
    @Volatile private var activeSocket: Socket? = null

    /** Listener owned by the worker, closed on stop/restart to release the port immediately. */
    @Volatile private var serverSocket: ServerSocket? = null

    /** Incremented per start so a superseded worker exits instead of competing for the port. */
    @Volatile private var sessionGeneration = 0

    @Volatile private var latestGyro = FloatArray(3)
    @Volatile private var latestGravity: FloatArray? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) {
            stopSelf()
            return START_NOT_STICKY
        }

        // A restart with no configuration cannot know which direction to use; stop cleanly
        // rather than silently picking one.
        if (intent == null && workerThread == null) {
            stopSelf()
            return START_NOT_STICKY
        }

        val targetHost = intent?.getStringExtra(EXTRA_HOST)?.takeIf { it.isNotBlank() }
        val targetPort = intent?.getIntExtra(EXTRA_PORT, Protocol.DEFAULT_PORT) ?: Protocol.DEFAULT_PORT
        val pairSecret = intent?.getStringExtra(EXTRA_PAIR_SECRET)

        startForegroundNotification()

        if (running.compareAndSet(false, true)) {
            acquireWakeLock()
            registerSensors()
            workerThread = Thread {
                runNetworkLoop(targetHost, targetPort, pairSecret, sessionGeneration)
            }.apply {
                name = "kineanchor-stream"
                start()
            }
        } else if (targetHost != null || pairSecret != null) {
            // Repeated start intents replace the session instead of stacking a second one.
            restartWorker(targetHost, targetPort, pairSecret)
        }
        return START_STICKY
    }

    /** Stops the current session completely, then starts one worker for the new direction. */
    private fun restartWorker(targetHost: String?, targetPort: Int, pairSecret: String?) {
        sessionGeneration++
        running.set(false)
        // Closing the listener and the live socket is what actually unblocks accept()/readLine().
        runCatching { serverSocket?.close() }
        serverSocket = null
        runCatching { activeSocket?.close() }
        activeSocket = null
        workerThread?.let { worker ->
            worker.interrupt()
            runCatching { worker.join(1_500) }
        }
        workerThread = null
        sampleQueue.clear()
        running.set(true)
        workerThread = Thread {
            runNetworkLoop(targetHost, targetPort, pairSecret, sessionGeneration)
        }.apply {
            name = "kineanchor-stream"
            start()
        }
    }

    override fun onDestroy() {
        running.set(false)
        sessionGeneration++
        runCatching { serverSocket?.close() }
        serverSocket = null
        runCatching { activeSocket?.close() }
        activeSocket = null
        workerThread?.let { worker ->
            worker.interrupt()
            runCatching { worker.join(1_500) }
        }
        workerThread = null
        sampleQueue.clear()
        sensorManager?.unregisterListener(this)
        sensorManager = null
        wakeLock?.let { if (it.isHeld) it.release() }
        wakeLock = null
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

    private fun runNetworkLoop(
        targetHost: String?,
        port: Int,
        initialSecret: String?,
        generation: Int,
    ) {
        val prefs = getSharedPreferences("kineanchor_prefs", Context.MODE_PRIVATE)
        var currentSecret = initialSecret

        // Exactly one direction per session: a scanned host means the phone dials the desktop,
        // no host means the phone listens for the desktop. Never both.
        val server = if (targetHost == null) {
            runCatching {
                ServerSocket().apply {
                    reuseAddress = true
                    bind(InetSocketAddress(port))
                    soTimeout = 500
                }
            }.getOrNull()
        } else {
            null
        }

        if (targetHost == null && server == null) {
            running.set(false)
            return
        }
        serverSocket = server

        try {
            while (running.get() && generation == sessionGeneration) {
                val sock: Socket? = if (server != null) {
                    runCatching { server.accept() }.getOrNull()
                } else {
                    runCatching {
                        Socket().apply {
                            connect(InetSocketAddress(targetHost, port), 400)
                        }
                    }.getOrNull()
                }

                if (sock == null) {
                    Thread.sleep(250)
                    continue
                }

                // One session at a time: a new peer replaces the old one instead of being
                // queued behind a peer that never disconnects.
                runCatching { activeSocket?.close() }
                activeSocket = sock
                try {
                    serveSession(sock, prefs, currentSecret)
                    currentSecret = null
                } finally {
                    runCatching { sock.close() }
                    if (activeSocket === sock) activeSocket = null
                }
            }
        } catch (_: InterruptedException) {
            Thread.currentThread().interrupt()
        } finally {
            runCatching { server?.close() }
        }
    }

    /**
     * Runs one authenticated session to completion.
     *
     * A separate reader thread watches the peer: without it, a peer that stops reading (cable
     * pulled, app killed) never produces a write error and the writer would spin forever. On EOF
     * or read failure the socket is closed, which makes the next write fail and ends the session.
     */
    private fun serveSession(
        sock: Socket,
        prefs: android.content.SharedPreferences,
        pairSecret: String?,
    ) {
        runCatching {
            sock.tcpNoDelay = true
            sock.soTimeout = 1_500
        }

        val savedToken = prefs.getString("paired_token", null)
        val writer = OutputStreamWriter(sock.getOutputStream(), Charsets.UTF_8)
        val reader = BufferedReader(InputStreamReader(sock.getInputStream(), Charsets.UTF_8))

        val hello = Protocol.Message.Hello(
            device = Build.MODEL ?: "Android",
            token = if (pairSecret == null) savedToken else null,
            pair = pairSecret,
        )
        writer.write(Protocol.encode(hello))
        writer.flush()

        val welcomeLine = runCatching { reader.readLine() }.getOrNull() ?: return
        val welcome = Protocol.decode(welcomeLine) as? Protocol.Message.Welcome ?: return

        prefs.edit()
            .putString("paired_token", welcome.token)
            .putString("paired_desktop", welcome.name)
            .apply()

        // Detects peer loss; closing here ends the writer loop on the next flush.
        val watchdog = Thread {
            runCatching {
                while (running.get()) {
                    val line = reader.readLine() ?: break
                    // Ping/pong keep the session alive; anything else is ignored.
                    if (line.isNotEmpty()) continue
                }
            }
            runCatching { sock.close() }
        }.apply {
            name = "kineanchor-peer"
            isDaemon = true
            start()
        }

        streamSamples(writer)
        runCatching { sock.close() }
        runCatching { watchdog.join(500) }
    }

    private fun streamSamples(writer: OutputStreamWriter) {
        while (running.get()) {
            val sample = sampleQueue.poll(500, TimeUnit.MILLISECONDS)
            val msg = sample ?: Protocol.Message.Ping
            // A failed write ends this session so the loop reconnects with a fresh socket.
            if (runCatching {
                    writer.write(Protocol.encode(msg))
                    writer.flush()
                }.isFailure
            ) {
                sampleQueue.clear()
                return
            }
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
