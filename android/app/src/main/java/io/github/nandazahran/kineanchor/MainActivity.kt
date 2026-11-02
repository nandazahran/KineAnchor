// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.nandazahran.kineanchor

import android.Manifest
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.core.content.ContextCompat
import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.DecodeHintType
import com.google.zxing.MultiFormatReader
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.common.HybridBinarizer
import java.util.concurrent.Executors

class MainActivity : AppCompatActivity() {
    private var selectedVehicle = 0 // 0 = Car, 1 = Ship
    private var selectedSource = 0 // 0 = Sensor, 1 = Synthetic

    private lateinit var overlayStatus: TextView
    private lateinit var streamStatus: TextView
    private lateinit var uriInput: EditText
    private lateinit var previewView: PreviewView
    private lateinit var scanButton: Button

    private var cameraProvider: ProcessCameraProvider? = null
    private var analysisExecutor = Executors.newSingleThreadExecutor()
    private var scannerActive = false

    // Launch exactly one StreamService per successful scan, even if several frames decode.
    private var scannedPairingHandled = false

    private val requestCameraPermission =
        registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
            if (granted) {
                startQrScan()
            } else {
                streamStatus.text = "Camera permission denied: enter the motion:// URI manually."
                stopQrScan()
            }
        }

    private val requestNotificationPermission =
        registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
            // Notifications are convenience only; denial must not affect streaming.
            if (!granted) {
                streamStatus.append("\n(Notifications blocked: streaming still works.)")
            }
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val prefs = getSharedPreferences("kineanchor_prefs", Context.MODE_PRIVATE)
        val scroll = ScrollView(this)
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(48, 48, 48, 48)
        }
        scroll.addView(root)

        // --- Section A: Phone On-Screen Overlay ---
        val overlayHeader = TextView(this).apply {
            text = "Mode A: Overlay on This Phone"
            textSize = 20f
        }
        overlayStatus = TextView(this)
        refreshOverlayPermissionStatus()

        val grantOverlayBtn = Button(this).apply {
            text = "Grant Display Over Other Apps Permission"
            setOnClickListener {
                if (!Settings.canDrawOverlays(this@MainActivity)) {
                    val intent = Intent(
                        Settings.ACTION_MANAGE_OVERLAY_PERMISSION,
                        Uri.parse("package:$packageName"),
                    )
                    startActivity(intent)
                } else {
                    overlayStatus.text = "Overlay permission: Granted"
                }
            }
        }
        val vehicleToggleBtn = Button(this).apply {
            text = "Vehicle: Car (tap to switch to Ship)"
            setOnClickListener {
                selectedVehicle = if (selectedVehicle == 0) 1 else 0
                text = if (selectedVehicle == 0) {
                    "Vehicle: Car (tap to switch to Ship)"
                } else {
                    "Vehicle: Ship (0.1-0.3Hz wave filter)"
                }
            }
        }
        val sourceToggleBtn = Button(this).apply {
            text = "Source: Local IMU Sensors"
            setOnClickListener {
                selectedSource = if (selectedSource == 0) 1 else 0
                text = if (selectedSource == 0) {
                    "Source: Local IMU Sensors"
                } else {
                    "Source: Synthetic 0.2Hz Sway (Experimental)"
                }
            }
        }
        val startOverlayBtn = Button(this).apply {
            text = "Start Phone Overlay"
            setOnClickListener {
                if (!NativeEngine.available) {
                    overlayStatus.text =
                        "Native engine failed to load: ${NativeEngine.loadError}"
                    return@setOnClickListener
                }
                if (!Settings.canDrawOverlays(this@MainActivity)) {
                    overlayStatus.text = "Please grant overlay permission first"
                    return@setOnClickListener
                }
                val intent = Intent(this@MainActivity, OverlayService::class.java).apply {
                    action = OverlayService.ACTION_START
                    putExtra(OverlayService.EXTRA_VEHICLE, selectedVehicle)
                    putExtra(OverlayService.EXTRA_SOURCE, selectedSource)
                }
                startForegroundService(intent)
                overlayStatus.text = "Phone overlay running (alpha <= 0.8 touch pass-through)"
            }
        }
        val zeroOverlayBtn = Button(this).apply {
            text = "Zero / Recalibrate Orientation"
            setOnClickListener {
                val intent = Intent(this@MainActivity, OverlayService::class.java).apply {
                    action = OverlayService.ACTION_ZERO
                }
                startService(intent)
            }
        }
        val stopOverlayBtn = Button(this).apply {
            text = "Stop Phone Overlay"
            setOnClickListener {
                val intent = Intent(this@MainActivity, OverlayService::class.java).apply {
                    action = OverlayService.ACTION_STOP
                }
                startService(intent)
                overlayStatus.text = "Phone overlay stopped"
            }
        }

        // --- Section B: Stream to PC (USB Tether / LAN Sensor Puck) ---
        val streamHeader = TextView(this).apply {
            text = "Mode B: Stream to PC (Sensor Puck)"
            textSize = 20f
            setPadding(0, 36, 0, 0)
        }
        streamStatus = TextView(this).apply {
            text = detectUsbStateText()
        }
        val tetherBtn = Button(this).apply {
            text = "1. Open USB Tethering Settings"
            setOnClickListener { openTetherSettings() }
        }

        previewView = PreviewView(this).apply {
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                640,
            )
            visibility = android.view.View.GONE
        }
        scanButton = Button(this).apply {
            text = "2. Scan Desktop QR"
            setOnClickListener { ensureCameraThenScan() }
        }

        uriInput = EditText(this).apply {
            hint = "motion://192.168.x.x:47474?pair=... (or scan QR)"
            setText(prefs.getString("last_uri", ""))
        }
        val startBtn = Button(this).apply {
            text = "3. Pair & Start 50Hz Sensor Stream"
            setOnClickListener {
                val raw = uriInput.text.toString().trim()
                if (raw.startsWith("motion://")) {
                    val parsed = runCatching { Protocol.parsePairingUri(raw) }.getOrNull()
                    if (parsed == null) {
                        streamStatus.text = "Invalid pairing URI: expected motion://host:port?pair=..."
                        return@setOnClickListener
                    }
                    prefs.edit().putString("last_uri", raw).apply()
                    startStream(parsed.host, parsed.port, parsed.pairSecret)
                } else {
                    // No host: this phone listens and the desktop dials over USB tether.
                    startStream(null, Protocol.DEFAULT_PORT, null)
                }
            }
        }
        val stopBtn = Button(this).apply {
            text = "Stop Stream"
            setOnClickListener {
                val intent = Intent(this@MainActivity, StreamService::class.java).apply {
                    action = StreamService.ACTION_STOP
                }
                startService(intent)
                streamStatus.text = "Stream stopped"
            }
        }

        root.addView(overlayHeader)
        root.addView(overlayStatus)
        root.addView(grantOverlayBtn)
        root.addView(vehicleToggleBtn)
        root.addView(sourceToggleBtn)
        root.addView(startOverlayBtn)
        root.addView(zeroOverlayBtn)
        root.addView(stopOverlayBtn)
        root.addView(streamHeader)
        root.addView(streamStatus)
        root.addView(tetherBtn)
        root.addView(previewView)
        root.addView(scanButton)
        root.addView(uriInput)
        root.addView(startBtn)
        root.addView(stopBtn)
        setContentView(scroll)

        refreshOverlayPermissionStatus()
        maybeRequestNotificationPermission()
    }

    override fun onResume() {
        super.onResume()
        // Overlay permission can change while this activity is in the background.
        if (::overlayStatus.isInitialized) {
            refreshOverlayPermissionStatus()
        }
    }

    override fun onDestroy() {
        stopQrScan()
        analysisExecutor.shutdown()
        super.onDestroy()
    }

    private fun refreshOverlayPermissionStatus() {
        // A missing native library blocks the overlay regardless of the permission, so that
        // error must win over the permission line.
        if (!NativeEngine.available) {
            overlayStatus.text = "Native engine failed to load: ${NativeEngine.loadError}"
            return
        }
        val granted = Settings.canDrawOverlays(this)
        overlayStatus.text = if (granted) {
            "Overlay permission: Granted"
        } else {
            "Overlay permission: Required (tap Grant below)"
        }
    }

    private fun maybeRequestNotificationPermission() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return
        val granted = ContextCompat.checkSelfPermission(
            this,
            Manifest.permission.POST_NOTIFICATIONS,
        ) == PackageManager.PERMISSION_GRANTED
        if (!granted) {
            requestNotificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
    }

    private fun ensureCameraThenScan() {
        if (scannerActive) {
            stopQrScan()
            return
        }
        if (packageManager.hasSystemFeature(PackageManager.FEATURE_CAMERA_ANY).not()) {
            streamStatus.text = "No camera on this device: enter the motion:// URI manually."
            return
        }
        val granted = ContextCompat.checkSelfPermission(
            this,
            Manifest.permission.CAMERA,
        ) == PackageManager.PERMISSION_GRANTED
        if (granted) {
            startQrScan()
        } else {
            requestCameraPermission.launch(Manifest.permission.CAMERA)
        }
    }

    private fun startQrScan() {
        val providerFuture = ProcessCameraProvider.getInstance(this)
        providerFuture.addListener({
            val provider = runCatching { providerFuture.get() }.getOrNull()
            if (provider == null) {
                streamStatus.text = "Camera unavailable: enter the motion:// URI manually."
                return@addListener
            }
            // `hasSystemFeature` reports the emulator's camera feature even when no camera can be
            // opened, so ask the provider. Failures here are synchronous and user-visible.
            val hasCamera = runCatching {
                provider.hasCamera(CameraSelector.DEFAULT_BACK_CAMERA) ||
                    provider.hasCamera(CameraSelector.DEFAULT_FRONT_CAMERA)
            }.getOrDefault(false)
            if (!hasCamera) {
                streamStatus.text = "No camera available: enter the motion:// URI manually."
                provider.unbindAll()
                return@addListener
            }
            cameraProvider = provider
            val preview = Preview.Builder().build().also {
                it.surfaceProvider = previewView.surfaceProvider
            }
            val analysis = ImageAnalysis.Builder()
                .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                .build()
            analysis.setAnalyzer(analysisExecutor) { image -> analyzeFrame(image) }

            val camera = runCatching {
                provider.unbindAll()
                provider.bindToLifecycle(
                    this,
                    CameraSelector.DEFAULT_BACK_CAMERA,
                    preview,
                    analysis,
                )
            }.getOrElse { error ->
                streamStatus.text = "Camera start failed: ${error.message}"
                stopQrScan()
                return@addListener
            }

            // CameraX validates cameras asynchronously, so a bind can succeed on a device whose
            // camera then fails to open. Surface that instead of pointing at a dead preview.
            camera.cameraInfo.cameraState.observe(this) { state ->
                if (state.type == androidx.camera.core.CameraState.Type.CLOSED && state.error != null) {
                    streamStatus.text = "Camera unavailable: ${state.error!!.code}"
                    stopQrScan()
                }
            }

            scannedPairingHandled = false
            scannerActive = true
            previewView.visibility = android.view.View.VISIBLE
            scanButton.text = "Cancel QR Scan"
            streamStatus.text = "Point the camera at the desktop pairing QR..."
        }, ContextCompat.getMainExecutor(this))
    }

    private fun stopQrScan() {
        scannerActive = false
        if (::previewView.isInitialized) {
            previewView.visibility = android.view.View.GONE
        }
        if (::scanButton.isInitialized) {
            scanButton.text = "2. Scan Desktop QR"
        }
        cameraProvider?.unbindAll()
        cameraProvider = null
        if (analysisExecutor.isShutdown) {
            analysisExecutor = Executors.newSingleThreadExecutor()
        }
    }

    private fun analyzeFrame(image: ImageProxy) {
        try {
            if (!scannerActive || scannedPairingHandled) return
            val uri = decodeQr(image) ?: return
            if (!uri.startsWith("motion://")) {
                runOnUiThread {
                    streamStatus.text = "Invalid QR: not a KineAnchor motion:// pairing code."
                }
                return
            }
            val parsed = runCatching { Protocol.parsePairingUri(uri) }.getOrNull()
            if (parsed == null) {
                runOnUiThread {
                    streamStatus.text = "Invalid QR: missing host, port or pair secret."
                }
                return
            }
            // Latch before touching the UI: later frames must not start a second session.
            scannedPairingHandled = true
            runOnUiThread {
                uriInput.setText(uri)
                getSharedPreferences("kineanchor_prefs", Context.MODE_PRIVATE)
                    .edit()
                    .putString("last_uri", uri)
                    .apply()
                stopQrScan()
                startStream(parsed.host, parsed.port, parsed.pairSecret)
            }
        } finally {
            image.close()
        }
    }

    private fun startStream(host: String?, port: Int, pairSecret: String?) {
        if (!NativeEngine.available) {
            streamStatus.text = "Native engine failed to load: ${NativeEngine.loadError}"
            return
        }
        val intent = Intent(this, StreamService::class.java).apply {
            action = StreamService.ACTION_START
            if (host != null) putExtra(StreamService.EXTRA_HOST, host)
            putExtra(StreamService.EXTRA_PORT, port)
            if (pairSecret != null) putExtra(StreamService.EXTRA_PAIR_SECRET, pairSecret)
        }
        startForegroundService(intent)
        streamStatus.text = when {
            host != null -> "Streaming to $host:$port (phone dials desktop)"
            else -> "Listening on port $port (desktop dials phone over USB tether)"
        }
    }

    private fun detectUsbStateText(): String {
        val sticky = registerReceiver(null, IntentFilter("android.hardware.usb.action.USB_STATE"))
        val connected = sticky?.getBooleanExtra("connected", false) ?: false
        val rndis = sticky?.getBooleanExtra("rndis", false) ?: false
        val ncm = sticky?.getBooleanExtra("ncm", false) ?: false
        return when {
            rndis || ncm -> "Status: USB Tethering active"
            connected -> "Status: USB connected — tap step 1 to enable USB tethering"
            else -> "Status: No USB data connection (plug cable; avoid charge-only cables)"
        }
    }

    private fun openTetherSettings() {
        val intents = listOf(
            Intent().apply {
                component = ComponentName("com.android.settings", "com.android.settings.TetherSettings")
            },
            Intent(Settings.ACTION_WIRELESS_SETTINGS),
            Intent(Settings.ACTION_SETTINGS),
        )
        for (candidate in intents) {
            if (runCatching { startActivity(candidate) }.isSuccess) break
        }
    }

    companion object {
        /**
         * Offline QR decode of the luminance plane from a CameraX [ImageProxy].
         * Returns the decoded text, or null when the frame holds no QR code.
         */
        fun decodeQr(image: ImageProxy): String? {
            val plane = image.planes.firstOrNull() ?: return null
            val buffer = plane.buffer
            val data = ByteArray(buffer.remaining())
            buffer.get(data)
            val source = PlanarYUVLuminanceSource(
                data,
                image.width,
                image.height,
                0,
                0,
                image.width,
                image.height,
                false,
            )
            val hints = mapOf(DecodeHintType.POSSIBLE_FORMATS to listOf(BarcodeFormat.QR_CODE))
            return runCatching {
                MultiFormatReader().apply { setHints(hints) }
                    .decode(BinaryBitmap(HybridBinarizer(source)))
                    .text
            }.getOrNull()
        }
    }
}
