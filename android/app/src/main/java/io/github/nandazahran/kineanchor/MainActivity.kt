// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.nandazahran.kineanchor

import android.Manifest
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.hardware.Sensor
import android.hardware.SensorManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.text.InputType
import android.util.TypedValue
import android.view.View
import android.view.ViewGroup
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ScrollView
import androidx.activity.OnBackPressedCallback
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.camera.core.CameraSelector
import androidx.camera.core.CameraState
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.core.content.ContextCompat
import androidx.core.view.WindowInsetsControllerCompat
import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.DecodeHintType
import com.google.zxing.MultiFormatReader
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.common.HybridBinarizer
import java.util.concurrent.Executors
import kotlin.math.abs

class MainActivity : AppCompatActivity() {
    private enum class Screen {
        MAIN,
        PAIR,
        PERMISSION,
        SETTINGS,
    }

    private enum class AppMode {
        OVERLAY,
        STREAM,
    }

    private data class ComfortPreset(
        val friendlyLabel: String,
        val literalLabel: String,
        val gain: Float,
        val opacity: Float,
        val density: Float,
    )

    private val presets = listOf(
        ComfortPreset("Gentle", "0.7x Gentle", 0.7f, 0.5f, 0.8f),
        ComfortPreset("Balanced", "1.0x Balanced", 1.0f, 0.65f, 1.0f),
        ComfortPreset("Winding", "1.6x Winding", 1.6f, 0.7f, 1.3f),
    )

    private lateinit var palette: V2Palette
    private lateinit var rootFrame: FrameLayout
    private var currentScreen = Screen.MAIN
    private var appMode = AppMode.OVERLAY
    private var settingsTab = 0 // 0 = Preferences, 1 = Privacy, 2 = Licenses

    private var showAdvancedInfo = false
    private var selectedVehicle = 0 // 0 = Car, 1 = Ship
    private var selectedSource = 0 // 0 = Local IMU, 1 = Demo 0.20 Hz
    private var gain = 1.0f
    private var dotRadius = 10.0f
    private var density = 1.0f
    private var opacity = 0.65f

    private var hasAccelSensor = true
    private var hasGyroSensor = true

    private var scopeView: HorizonScopeView? = null
    private var mainStatusSlot: LinearLayout? = null
    private var mainDynamicSlot: LinearLayout? = null
    private var mainPresetSlot: LinearLayout? = null
    private var pairDynamicSlot: LinearLayout? = null
    private var uriInput: EditText? = null
    private var previewView: PreviewView? = null
    private var scanErrorText: String? = null
    private var confirmForget = false

    private var cameraProvider: ProcessCameraProvider? = null
    private var analysisExecutor = Executors.newSingleThreadExecutor()
    private var scannerActive = false
    private var scannedPairingHandled = false

    private val uiHandler = Handler(Looper.getMainLooper())
    private var lastRenderedFingerprint = ""
    private var lastPresetFingerprint = ""
    private val refreshTick = object : Runnable {
        override fun run() {
            syncLiveState()
            uiHandler.postDelayed(this, 250L)
        }
    }

    private val requestCameraPermission =
        registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
            if (granted) {
                scanErrorText = null
                startQrScan()
            } else {
                scanErrorText = "Camera permission denied. Paste a motion:// URI below."
                stopQrScan()
                renderScreen()
            }
        }

    private val requestNotificationPermission =
        registerForActivityResult(ActivityResultContracts.RequestPermission()) { _ ->
            // Notifications are convenience only; denial never blocks overlay or stream.
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        palette = V2Palette.forContext(this)
        applySystemBars()
        loadConfig()
        probeSensors()

        rootFrame = FrameLayout(this).apply {
            setBackgroundColor(palette.bg)
        }
        setContentView(rootFrame)

        onBackPressedDispatcher.addCallback(
            this,
            object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    if (currentScreen != Screen.MAIN) {
                        navigateTo(Screen.MAIN)
                    } else {
                        isEnabled = false
                        onBackPressedDispatcher.onBackPressed()
                    }
                }
            },
        )

        renderScreen()
        maybeRequestNotificationPermission()
    }

    override fun onResume() {
        super.onResume()
        palette = V2Palette.forContext(this)
        applySystemBars()
        rootFrame.setBackgroundColor(palette.bg)
        renderScreen()
        uiHandler.removeCallbacks(refreshTick)
        uiHandler.postDelayed(refreshTick, 250L)
    }

    override fun onPause() {
        uiHandler.removeCallbacks(refreshTick)
        super.onPause()
    }

    override fun onDestroy() {
        uiHandler.removeCallbacks(refreshTick)
        stopQrScan()
        analysisExecutor.shutdown()
        super.onDestroy()
    }

    private fun applySystemBars() {
        @Suppress("DEPRECATION")
        window.statusBarColor = palette.bg
        @Suppress("DEPRECATION")
        window.navigationBarColor = palette.bg
        val lightChrome = palette == V2Palette.Light
        WindowInsetsControllerCompat(window, window.decorView).apply {
            isAppearanceLightStatusBars = lightChrome
            isAppearanceLightNavigationBars = lightChrome
        }
    }

    private fun prefs() = getSharedPreferences("kineanchor_prefs", Context.MODE_PRIVATE)

    private fun loadConfig() {
        val p = prefs()
        showAdvancedInfo = p.getBoolean("show_advanced_info", false)
        selectedVehicle = p.getInt("vehicle", 0)
        selectedSource = p.getInt("source", 0)
        density = p.getFloat("density", 1.0f)
        dotRadius = p.getFloat("radius", 10.0f)
        opacity = p.getFloat("opacity", 0.65f).coerceAtMost(0.8f)
        gain = p.getFloat("gain", 1.0f)
    }

    private fun saveConfig() {
        prefs().edit()
            .putBoolean("show_advanced_info", showAdvancedInfo)
            .putInt("vehicle", selectedVehicle)
            .putInt("source", selectedSource)
            .putFloat("density", density)
            .putFloat("radius", dotRadius)
            .putFloat("opacity", opacity.coerceAtMost(0.8f))
            .putFloat("gain", gain)
            .apply()
        if (OverlayService.currentState.running) {
            val intent = Intent(this, OverlayService::class.java).apply {
                action = OverlayService.ACTION_UPDATE_CONFIG
                putExtra(OverlayService.EXTRA_VEHICLE, selectedVehicle)
                putExtra(OverlayService.EXTRA_SOURCE, selectedSource)
                putExtra(OverlayService.EXTRA_DENSITY, density)
                putExtra(OverlayService.EXTRA_RADIUS, dotRadius)
                putExtra(OverlayService.EXTRA_OPACITY, opacity.coerceAtMost(0.8f))
                putExtra(OverlayService.EXTRA_GAIN, gain)
            }
            startService(intent)
        }
    }

    private fun resetDefaults() {
        selectedVehicle = 0
        selectedSource = 0
        density = 1.0f
        dotRadius = 10.0f
        opacity = 0.65f
        gain = 1.0f
        saveConfig()
    }

    private fun probeSensors() {
        val sm = getSystemService(Context.SENSOR_SERVICE) as SensorManager
        hasAccelSensor = sm.getDefaultSensor(Sensor.TYPE_LINEAR_ACCELERATION) != null
        hasGyroSensor = sm.getDefaultSensor(Sensor.TYPE_GYROSCOPE) != null
    }

    private fun presetIndex(): Int =
        presets.indexOfFirst {
            abs(it.gain - gain) < 0.05f &&
                abs(it.opacity - opacity) < 0.05f &&
                abs(it.density - density) < 0.05f
        }

    private fun navigateTo(next: Screen) {
        if (currentScreen == Screen.PAIR && next != Screen.PAIR) {
            stopQrScan()
        }
        confirmForget = false
        currentScreen = next
        renderScreen()
    }

    private fun renderScreen() {
        val p = palette
        rootFrame.removeAllViews()
        mainStatusSlot = null
        mainDynamicSlot = null
        mainPresetSlot = null
        pairDynamicSlot = null
        scopeView = null
        lastRenderedFingerprint = ""
        lastPresetFingerprint = ""

        val container = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT,
            )
        }

        val header = when (currentScreen) {
            Screen.MAIN -> V2Widgets.topHeader(
                context = this,
                p = p,
                title = "KineAnchor",
                leadingIcon = R.drawable.ic_lucide_compass,
                trailingActions = listOf(
                    R.drawable.ic_lucide_sliders_horizontal to ("Settings" to { navigateTo(Screen.SETTINGS) }),
                ),
            )
            Screen.PAIR -> V2Widgets.topHeader(
                context = this,
                p = p,
                title = "Pair laptop",
                onBack = { navigateTo(Screen.MAIN) },
            )
            Screen.PERMISSION -> V2Widgets.topHeader(
                context = this,
                p = p,
                title = "Overlay permission",
                onBack = { navigateTo(Screen.MAIN) },
            )
            Screen.SETTINGS -> V2Widgets.topHeader(
                context = this,
                p = p,
                title = "Settings",
                onBack = { navigateTo(Screen.MAIN) },
            )
        }
        container.addView(header)
        container.addView(V2Widgets.hairline(this, p))

        val body = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(
                UiKit.dp(context, 16f),
                UiKit.dp(context, 14f),
                UiKit.dp(context, 16f),
                UiKit.dp(context, 24f),
            )
        }
        when (currentScreen) {
            Screen.MAIN -> buildMain(body, p)
            Screen.PAIR -> buildPair(body, p)
            Screen.PERMISSION -> buildPermission(body, p)
            Screen.SETTINGS -> buildSettings(body, p)
        }

        val scroll = ScrollView(this).apply {
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                0,
                1f,
            )
            addView(body)
        }
        container.addView(scroll)
        rootFrame.addView(container)
        syncLiveState(forceRebuild = true)
    }

    private fun buildMain(body: LinearLayout, p: V2Palette) {
        val adv = showAdvancedInfo

        val scope = HorizonScopeView(this, p).apply {
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                UiKit.dp(context, 136f),
            )
        }
        scopeView = scope
        body.addView(scope)
        body.addView(V2Widgets.vSpace(this, 12f))

        body.addView(V2Widgets.sectionCaption(this, p, if (adv) "Mode" else "Target"))
        body.addView(
            V2Widgets.segmented(
                context = this,
                p = p,
                options = if (adv) listOf("Local overlay", "TCP stream") else listOf("Phone screen", "Stream to PC"),
                selected = if (appMode == AppMode.OVERLAY) 0 else 1,
                onSelect = { idx ->
                    appMode = if (idx == 0) AppMode.OVERLAY else AppMode.STREAM
                    renderScreen()
                },
            ),
        )
        body.addView(V2Widgets.vSpace(this, 12f))

        val statusSlot = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
        }
        mainStatusSlot = statusSlot
        body.addView(statusSlot)

        val dynamicSlot = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
        }
        mainDynamicSlot = dynamicSlot
        body.addView(dynamicSlot)

        // Unified Motion & cues / Filter & cues section (present in both friendly and advanced modes)
        body.addView(V2Widgets.vSpace(this, 14f))
        body.addView(
            V2Widgets.sectionCaption(
                this,
                p,
                if (adv) "Filter & cues · omega 6.0 · zeta 0.70" else "Motion & cues",
            ),
        )

        val presetSlot = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
        }
        mainPresetSlot = presetSlot
        body.addView(presetSlot)
        rebuildPresetAndVehicleSlot(p)

        body.addView(V2Widgets.vSpace(this, 10f))
        val sliderCard = V2Widgets.cardColumn(this, p, padV = 6f)
        sliderCard.addView(
            V2Widgets.sliderRow(
                context = this,
                p = p,
                label = if (adv) "Gain" else "Strength",
                minVal = 0.3f,
                maxVal = 2.5f,
                initial = gain,
                formatValue = { if (adv) String.format("%.2fx", it) else strengthLabel(it) },
                onValue = {
                    gain = it
                    saveConfig()
                    refreshPresetSlotIfChanged()
                },
            ),
        )
        sliderCard.addView(V2Widgets.hairline(this, p))
        sliderCard.addView(
            V2Widgets.sliderRow(
                context = this,
                p = p,
                label = if (adv) "Dot radius" else "Dot size",
                minVal = 4.0f,
                maxVal = 18.0f,
                initial = dotRadius,
                formatValue = { if (adv) String.format("%.0f dp", it) else sizeLabel(it) },
                onValue = {
                    dotRadius = it
                    saveConfig()
                },
            ),
        )
        sliderCard.addView(V2Widgets.hairline(this, p))
        sliderCard.addView(
            V2Widgets.sliderRow(
                context = this,
                p = p,
                label = if (adv) "Dot density" else "Dot amount",
                minVal = 0.5f,
                maxVal = 2.0f,
                initial = density,
                formatValue = { if (adv) String.format("%.2fx", it) else densityLabel(it) },
                onValue = {
                    density = it
                    saveConfig()
                    refreshPresetSlotIfChanged()
                },
            ),
        )
        sliderCard.addView(V2Widgets.hairline(this, p))
        sliderCard.addView(
            V2Widgets.sliderRow(
                context = this,
                p = p,
                label = if (adv) "Opacity" else "Visibility",
                minVal = 0.2f,
                maxVal = 0.8f,
                initial = opacity,
                formatValue = {
                    if (adv) String.format("%.0f%% (max 80%%)", it * 100f) else visibilityLabel(it)
                },
                onValue = {
                    opacity = it.coerceAtMost(0.8f)
                    saveConfig()
                    refreshPresetSlotIfChanged()
                },
            ),
        )
        body.addView(sliderCard)
        if (adv) {
            body.addView(V2Widgets.vSpace(this, 8f))
            body.addView(
                V2Widgets.bodyText(
                    context = this,
                    p = p,
                    text = "Opacity caps at 0.80 so Android keeps touches active beneath the overlay.",
                    color = p.inkMuted,
                    sizeSp = 12f,
                ),
            )
        }
    }

    private fun refreshPresetSlotIfChanged() {
        val fp = "${presetIndex()}|$selectedVehicle|$showAdvancedInfo"
        if (fp != lastPresetFingerprint) {
            lastPresetFingerprint = fp
            rebuildPresetAndVehicleSlot(palette)
        }
    }

    private fun rebuildPresetAndVehicleSlot(p: V2Palette) {
        val slot = mainPresetSlot ?: return
        val adv = showAdvancedInfo
        lastPresetFingerprint = "${presetIndex()}|$selectedVehicle|$adv"
        slot.removeAllViews()

        slot.addView(V2Widgets.sectionCaption(this, p, if (adv) "Preset" else "Comfort"))
        slot.addView(
            V2Widgets.segmented(
                context = this,
                p = p,
                options = presets.map { if (adv) it.literalLabel else it.friendlyLabel },
                selected = presetIndex(),
                onSelect = { idx ->
                    val chosen = presets[idx]
                    gain = chosen.gain
                    opacity = chosen.opacity.coerceAtMost(0.8f)
                    density = chosen.density
                    saveConfig()
                    renderScreen()
                },
            ),
        )
        slot.addView(V2Widgets.vSpace(this, 10f))
        slot.addView(V2Widgets.sectionCaption(this, p, if (adv) "Filter mapping" else "Travel mode"))
        slot.addView(
            V2Widgets.segmented(
                context = this,
                p = p,
                options = if (adv) {
                    listOf("Force & tilt (Car)", "Full 6-DoF (0.1–0.3 Hz)")
                } else {
                    listOf("Car & bus", "Boat & ship")
                },
                selected = selectedVehicle,
                onSelect = { idx ->
                    selectedVehicle = idx
                    saveConfig()
                    rebuildPresetAndVehicleSlot(p)
                },
            ),
        )
    }

    private fun syncLiveState(forceRebuild: Boolean = false) {
        val overlay = OverlayService.currentState
        val stream = StreamService.state
        val overlayPerm = Settings.canDrawOverlays(this)
        val usb = probeUsbState()

        scopeView?.let { sv ->
            val st = resolveScopeStatus(overlay, stream, overlayPerm)
            sv.updateState(
                label = st.label,
                tone = st.tone,
                horizonX = overlay.horizonX,
                horizonY = overlay.horizonY,
                synthetic = st.synthetic,
                active = st.active,
                customReadout = st.readout,
            )
        }

        val fingerprint = buildString {
            append(currentScreen).append('|')
            append(appMode).append('|')
            append(showAdvancedInfo).append('|')
            append(overlay.running).append('|').append(overlay.paused).append('|')
            append(overlay.windowError).append('|')
            append(overlayPerm).append('|').append(selectedSource).append('|')
            append(stream).append('|')
            append(usb.first).append('|')
            append(scanErrorText).append('|').append(scannerActive).append('|').append(confirmForget)
        }

        if (!forceRebuild && fingerprint == lastRenderedFingerprint) return
        lastRenderedFingerprint = fingerprint

        if (currentScreen == Screen.MAIN) {
            rebuildMainStatusAndControls(overlay, stream, overlayPerm, usb)
        } else if (currentScreen == Screen.PAIR) {
            rebuildPairDynamic(stream)
        }
    }

    private data class ScopeStatus(
        val label: String,
        val tone: HorizonScopeView.ScopeTone,
        val synthetic: Boolean,
        val active: Boolean,
        val readout: String,
    )

    private fun resolveScopeStatus(
        overlay: OverlayService.StateSnapshot,
        stream: StreamService.StreamState,
        overlayPerm: Boolean,
    ): ScopeStatus {
        val adv = showAdvancedInfo
        if (!NativeEngine.available) {
            return ScopeStatus(
                if (adv) "Engine error · JNI" else "Engine error",
                HorizonScopeView.ScopeTone.ERR,
                false,
                false,
                if (adv) "0.00 / 0.00" else "Error",
            )
        }
        return if (appMode == AppMode.OVERLAY) {
            when {
                selectedSource == 0 && (!hasAccelSensor || !hasGyroSensor) ->
                    ScopeStatus(
                        "Missing sensors",
                        HorizonScopeView.ScopeTone.ERR,
                        false,
                        false,
                        if (adv) "0.00 / 0.00" else "Unavailable",
                    )
                overlay.windowError != null ->
                    ScopeStatus(
                        "Overlay blocked",
                        HorizonScopeView.ScopeTone.ERR,
                        false,
                        false,
                        if (adv) "0.00 / 0.00" else "Blocked",
                    )
                !overlayPerm ->
                    ScopeStatus(
                        "Permission needed",
                        HorizonScopeView.ScopeTone.WARN,
                        false,
                        false,
                        if (adv) "0.00 / 0.00" else "Setup",
                    )
                overlay.running && overlay.paused ->
                    ScopeStatus(
                        if (adv) "Paused · 0 Hz" else "Paused",
                        HorizonScopeView.ScopeTone.MUTED,
                        selectedSource == 1,
                        false,
                        if (adv) "0.00 / 0.00 · Paused" else "Static",
                    )
                overlay.running && selectedSource == 1 ->
                    ScopeStatus(
                        if (adv) "Demo · 0.20 Hz" else "Demo wave",
                        HorizonScopeView.ScopeTone.WARN,
                        true,
                        true,
                        if (adv) {
                            String.format("%+.2f / %+.2f · 60 Hz", overlay.horizonX, overlay.horizonY)
                        } else {
                            "Preview"
                        },
                    )
                overlay.running ->
                    ScopeStatus(
                        if (adv) "Active · 50 Hz" else "Active",
                        HorizonScopeView.ScopeTone.LIVE,
                        false,
                        true,
                        if (adv) {
                            String.format("%+.2f / %+.2f · 50 Hz", overlay.horizonX, overlay.horizonY)
                        } else {
                            "Steady"
                        },
                    )
                selectedSource == 1 ->
                    ScopeStatus(
                        if (adv) "Demo ready · 0.20 Hz" else "Demo ready",
                        HorizonScopeView.ScopeTone.WARN,
                        true,
                        false,
                        if (adv) "0.00 / 0.00" else "Idle",
                    )
                else ->
                    ScopeStatus(
                        if (adv) "Idle · 0 Hz" else "Idle",
                        HorizonScopeView.ScopeTone.MUTED,
                        false,
                        false,
                        if (adv) "0.00 / 0.00" else "Idle",
                    )
            }
        } else {
            when (stream) {
                is StreamService.StreamState.Streaming ->
                    ScopeStatus(
                        if (adv) "Streaming · 50 Hz" else "Streaming",
                        HorizonScopeView.ScopeTone.LIVE,
                        false,
                        true,
                        if (adv) "50 Hz · NDJSON" else "Live",
                    )
                is StreamService.StreamState.Listening ->
                    ScopeStatus(
                        if (adv) "Listening · :${stream.port}" else "Waiting for PC",
                        HorizonScopeView.ScopeTone.WARN,
                        false,
                        false,
                        if (adv) "TCP :${stream.port}" else "Listening",
                    )
                is StreamService.StreamState.Connecting ->
                    ScopeStatus(
                        "Connecting",
                        HorizonScopeView.ScopeTone.WARN,
                        false,
                        false,
                        if (adv) "${stream.host}:${stream.port}" else "Dialing",
                    )
                is StreamService.StreamState.Rejected ->
                    ScopeStatus(
                        if (adv) "Rejected" else "Pairing rejected",
                        HorizonScopeView.ScopeTone.ERR,
                        false,
                        false,
                        if (adv) "bad_token" else "Error",
                    )
                is StreamService.StreamState.BindError ->
                    ScopeStatus(
                        "Port busy",
                        HorizonScopeView.ScopeTone.ERR,
                        false,
                        false,
                        if (adv) "TCP :${Protocol.DEFAULT_PORT}" else "Blocked",
                    )
                is StreamService.StreamState.Disconnected ->
                    ScopeStatus(
                        "Disconnected",
                        HorizonScopeView.ScopeTone.WARN,
                        false,
                        false,
                        if (adv) "Socket closed" else "Offline",
                    )
                StreamService.StreamState.Idle ->
                    ScopeStatus(
                        if (adv) "Idle · 0 Hz" else "Idle",
                        HorizonScopeView.ScopeTone.MUTED,
                        false,
                        false,
                        if (adv) "0.00 / 0.00" else "Idle",
                    )
            }
        }
    }

    private fun rebuildMainStatusAndControls(
        overlay: OverlayService.StateSnapshot,
        stream: StreamService.StreamState,
        overlayPerm: Boolean,
        usb: Pair<String, Boolean>,
    ) {
        val p = palette
        val adv = showAdvancedInfo
        val statusSlot = mainStatusSlot ?: return
        val dynSlot = mainDynamicSlot ?: return
        statusSlot.removeAllViews()
        dynSlot.removeAllViews()

        // Priority system banners
        if (!NativeEngine.available) {
            statusSlot.addView(
                V2Widgets.noticeBox(
                    context = this,
                    p = p,
                    tone = V2Widgets.NoticeTone.ERR,
                    title = "Native engine unavailable",
                    body = "Could not load libkineanchor_jni.so (${NativeEngine.loadError ?: "ABI mismatch"}). Reinstall a matching build.",
                ),
            )
            statusSlot.addView(V2Widgets.vSpace(this, 10f))
        } else if (selectedSource == 0 && (!hasAccelSensor || !hasGyroSensor)) {
            val missing = buildList {
                if (!hasAccelSensor) add("linear accelerometer")
                if (!hasGyroSensor) add("gyroscope")
            }.joinToString(" and ")
            statusSlot.addView(
                V2Widgets.noticeBox(
                    context = this,
                    p = p,
                    tone = V2Widgets.NoticeTone.ERR,
                    title = "Required IMU sensor missing",
                    body = "This device lacks a $missing. Live tracking cannot run; switch to Demo to preview cues.",
                    actionLabel = "Use Demo",
                    onAction = {
                        selectedSource = 1
                        saveConfig()
                        renderScreen()
                    },
                ),
            )
            statusSlot.addView(V2Widgets.vSpace(this, 10f))
        }

        if (appMode == AppMode.OVERLAY) {
            if (overlay.windowError != null) {
                statusSlot.addView(
                    V2Widgets.noticeBox(
                        context = this,
                        p = p,
                        tone = V2Widgets.NoticeTone.ERR,
                        title = "Overlay could not attach",
                        body = overlay.windowError,
                        actionLabel = "Overlay settings",
                        onAction = { navigateTo(Screen.PERMISSION) },
                    ),
                )
                statusSlot.addView(V2Widgets.vSpace(this, 10f))
            } else if (!overlayPerm && NativeEngine.available) {
                statusSlot.addView(
                    V2Widgets.noticeBox(
                        context = this,
                        p = p,
                        tone = V2Widgets.NoticeTone.WARN,
                        title = "Display over apps required",
                        body = "Android requires overlay permission before dots can draw over other apps.",
                        actionLabel = "Grant access",
                        onAction = { navigateTo(Screen.PERMISSION) },
                    ),
                )
                statusSlot.addView(V2Widgets.vSpace(this, 10f))
            }

            if (selectedSource == 1) {
                statusSlot.addView(
                    V2Widgets.noticeBox(
                        context = this,
                        p = p,
                        tone = V2Widgets.NoticeTone.WARN,
                        title = if (adv) "Synthetic wave active (0.20 Hz)" else "Demo motion active",
                        body = if (adv) {
                            "Cues follow a synthetic 0.20 Hz wave instead of device sensors. Switch to Local IMU before travel."
                        } else {
                            "Cues show a sample wave instead of phone sensors. Switch back to Phone sensors before travel."
                        },
                        actionLabel = if (adv) "Switch to IMU" else "Use sensors",
                        onAction = {
                            selectedSource = 0
                            saveConfig()
                            renderScreen()
                        },
                    ),
                )
                statusSlot.addView(V2Widgets.vSpace(this, 10f))
            }

            // Primary action row
            val canStart = NativeEngine.available && overlayPerm &&
                (selectedSource == 1 || (hasAccelSensor && hasGyroSensor))

            val btnRow = LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
            }
            if (!overlay.running) {
                val startBtn = V2Widgets.actionButton(
                    context = this,
                    p = p,
                    label = if (adv) "Start overlay" else "Start cues",
                    kind = V2Widgets.ButtonKind.PRIMARY,
                    iconRes = R.drawable.ic_lucide_play,
                    enabled = canStart,
                    onClick = { startOverlayService() },
                ).apply {
                    layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
                }
                btnRow.addView(startBtn)
            } else {
                val pauseResumeBtn = V2Widgets.actionButton(
                    context = this,
                    p = p,
                    label = if (overlay.paused) "Resume" else "Pause",
                    kind = if (overlay.paused) V2Widgets.ButtonKind.PRIMARY else V2Widgets.ButtonKind.SECONDARY,
                    iconRes = if (overlay.paused) R.drawable.ic_lucide_play else R.drawable.ic_lucide_pause,
                    onClick = {
                        val action = if (overlay.paused) OverlayService.ACTION_RESUME else OverlayService.ACTION_PAUSE
                        startService(Intent(this, OverlayService::class.java).setAction(action))
                    },
                ).apply {
                    layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
                }
                val zeroBtn = V2Widgets.actionButton(
                    context = this,
                    p = p,
                    label = if (adv) "Zero" else "Center",
                    kind = V2Widgets.ButtonKind.SECONDARY,
                    iconRes = R.drawable.ic_lucide_crosshair,
                    onClick = {
                        startService(Intent(this, OverlayService::class.java).setAction(OverlayService.ACTION_ZERO))
                    },
                )
                val stopBtn = V2Widgets.actionButton(
                    context = this,
                    p = p,
                    label = "Stop",
                    kind = V2Widgets.ButtonKind.DANGER,
                    onClick = {
                        startService(Intent(this, OverlayService::class.java).setAction(OverlayService.ACTION_STOP))
                    },
                )
                btnRow.addView(pauseResumeBtn)
                btnRow.addView(V2Widgets.hSpace(this, 8f))
                btnRow.addView(zeroBtn)
                btnRow.addView(V2Widgets.hSpace(this, 8f))
                btnRow.addView(stopBtn)
            }
            dynSlot.addView(btnRow)
            dynSlot.addView(V2Widgets.vSpace(this, 14f))

            // Motion source selector (unified on main screen)
            dynSlot.addView(
                V2Widgets.sectionCaption(
                    this,
                    p,
                    if (adv) "Sensor source · 50 Hz" else "Motion source",
                ),
            )
            dynSlot.addView(
                V2Widgets.segmented(
                    context = this,
                    p = p,
                    options = if (adv) listOf("Local IMU", "Synthetic 0.20 Hz") else listOf("Phone sensors", "Demo wave"),
                    selected = selectedSource,
                    onSelect = { idx ->
                        selectedSource = idx
                        saveConfig()
                        renderScreen()
                    },
                ),
            )
        } else {
            // Stream to PC mode
            when (stream) {
                is StreamService.StreamState.Rejected -> {
                    statusSlot.addView(
                        V2Widgets.noticeBox(
                            context = this,
                            p = p,
                            tone = V2Widgets.NoticeTone.ERR,
                            title = "Pairing rejected",
                            body = "${stream.reason}. Scan a fresh desktop QR code or re-enter the motion:// URI.",
                            actionLabel = "Pair laptop",
                            onAction = { navigateTo(Screen.PAIR) },
                        ),
                    )
                    statusSlot.addView(V2Widgets.vSpace(this, 10f))
                }
                is StreamService.StreamState.BindError -> {
                    statusSlot.addView(
                        V2Widgets.noticeBox(
                            context = this,
                            p = p,
                            tone = V2Widgets.NoticeTone.ERR,
                            title = "Port ${Protocol.DEFAULT_PORT} unavailable",
                            body = "${stream.message}. Close conflicting apps or dial the desktop via QR code instead.",
                            actionLabel = "Pair laptop",
                            onAction = { navigateTo(Screen.PAIR) },
                        ),
                    )
                    statusSlot.addView(V2Widgets.vSpace(this, 10f))
                }
                is StreamService.StreamState.Disconnected -> {
                    statusSlot.addView(
                        V2Widgets.noticeBox(
                            context = this,
                            p = p,
                            tone = V2Widgets.NoticeTone.WARN,
                            title = "Stream interrupted",
                            body = stream.reason,
                            iconRes = R.drawable.ic_lucide_plug,
                        ),
                    )
                    statusSlot.addView(V2Widgets.vSpace(this, 10f))
                }
                else -> Unit
            }

            if (!usb.second && stream !is StreamService.StreamState.Streaming) {
                statusSlot.addView(
                    V2Widgets.noticeBox(
                        context = this,
                        p = p,
                        tone = V2Widgets.NoticeTone.WARN,
                        title = "USB tethering inactive",
                        body = "${usb.first}. Enable USB tethering for cable streaming, or pair over Wi-Fi QR.",
                        iconRes = R.drawable.ic_lucide_plug,
                        actionLabel = "Tether settings",
                        onAction = { openTetherSettings() },
                    ),
                )
                statusSlot.addView(V2Widgets.vSpace(this, 10f))
            }

            val streamActive = stream is StreamService.StreamState.Streaming ||
                stream is StreamService.StreamState.Listening ||
                stream is StreamService.StreamState.Connecting
            val canStream = NativeEngine.available && hasAccelSensor && hasGyroSensor

            val btnRow = LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
            }
            val primaryBtn = if (!streamActive) {
                V2Widgets.actionButton(
                    context = this,
                    p = p,
                    label = "Start stream",
                    kind = V2Widgets.ButtonKind.PRIMARY,
                    iconRes = R.drawable.ic_lucide_play,
                    enabled = canStream,
                    onClick = {
                        val lastUri = prefs().getString("last_uri", "")?.trim().orEmpty()
                        val parsed = if (lastUri.startsWith("motion://")) {
                            runCatching { Protocol.parsePairingUri(lastUri) }.getOrNull()
                        } else {
                            null
                        }
                        if (parsed != null) {
                            startStream(parsed.host, parsed.port, parsed.pairSecret)
                        } else {
                            startStream(null, Protocol.DEFAULT_PORT, null)
                        }
                    },
                )
            } else {
                V2Widgets.actionButton(
                    context = this,
                    p = p,
                    label = "Stop stream",
                    kind = V2Widgets.ButtonKind.DANGER,
                    onClick = {
                        startService(Intent(this, StreamService::class.java).setAction(StreamService.ACTION_STOP))
                    },
                )
            }.apply {
                layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
            }
            val pairBtn = V2Widgets.actionButton(
                context = this,
                p = p,
                label = if (adv) "Pair QR / URI" else "Pair laptop",
                kind = V2Widgets.ButtonKind.SECONDARY,
                iconRes = R.drawable.ic_lucide_qr_code,
                onClick = { navigateTo(Screen.PAIR) },
            )
            btnRow.addView(primaryBtn)
            btnRow.addView(V2Widgets.hSpace(this, 8f))
            btnRow.addView(pairBtn)
            dynSlot.addView(btnRow)
            dynSlot.addView(V2Widgets.vSpace(this, 14f))

            dynSlot.addView(
                V2Widgets.sectionCaption(
                    this,
                    p,
                    if (adv) "Transport · TCP :${Protocol.DEFAULT_PORT}" else "Laptop connection",
                ),
            )
            val card = V2Widgets.cardColumn(this, p, padV = 4f)
            val pairedName = prefs().getString("paired_desktop", null) ?: "Not paired"
            val streamText = when (stream) {
                is StreamService.StreamState.Streaming -> "Streaming to ${stream.peerName ?: stream.endpoint}"
                is StreamService.StreamState.Listening -> "Listening on :${stream.port}"
                is StreamService.StreamState.Connecting -> "Connecting to ${stream.host}:${stream.port}"
                else -> "Stopped"
            }
            card.addView(V2Widgets.infoRow(this, p, if (adv) "Link" else "Status", streamText, monoValue = adv))
            card.addView(V2Widgets.hairline(this, p))
            val usbVal = if (adv && usb.second) "${usb.first} · RNDIS" else usb.first
            card.addView(V2Widgets.infoRow(this, p, "USB tether", usbVal, monoValue = adv))
            card.addView(V2Widgets.hairline(this, p))
            card.addView(V2Widgets.infoRow(this, p, "Desktop", pairedName, monoValue = adv))
            dynSlot.addView(card)
        }
    }

    private fun buildPair(body: LinearLayout, p: V2Palette) {
        val dynSlot = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
        }
        pairDynamicSlot = dynSlot
        body.addView(dynSlot)

        val preview = PreviewView(this).apply {
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                UiKit.dp(context, 220f),
            )
            visibility = if (scannerActive) View.VISIBLE else View.GONE
            background = UiKit.boxDrawable(this@MainActivity, p.well, p.border, 8f)
        }
        previewView = preview
        body.addView(preview)
        if (scannerActive) body.addView(V2Widgets.vSpace(this, 10f))

        val scanBtn = V2Widgets.actionButton(
            context = this,
            p = p,
            label = if (scannerActive) "Cancel QR scan" else "Scan desktop QR",
            kind = if (scannerActive) V2Widgets.ButtonKind.SECONDARY else V2Widgets.ButtonKind.PRIMARY,
            iconRes = R.drawable.ic_lucide_qr_code,
            onClick = { ensureCameraThenScan() },
        )
        body.addView(scanBtn)

        body.addView(V2Widgets.vSpace(this, 14f))
        body.addView(V2Widgets.sectionCaption(this, p, "Manual URI"))
        val edit = EditText(this).apply {
            hint = "motion://192.168.1.10:47474?pair=..."
            setText(prefs().getString("last_uri", ""))
            setTextColor(p.ink)
            setHintTextColor(p.inkMuted)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 12.5f)
            typeface = UiKit.mono()
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_URI
            background = UiKit.boxDrawable(this@MainActivity, p.well, p.border, 6f)
            setPadding(
                UiKit.dp(context, 12f),
                UiKit.dp(context, 11f),
                UiKit.dp(context, 12f),
                UiKit.dp(context, 11f),
            )
        }
        uriInput = edit
        body.addView(edit)
        body.addView(V2Widgets.vSpace(this, 10f))

        val actionRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
        }
        val connectBtn = V2Widgets.actionButton(
            context = this,
            p = p,
            label = "Connect",
            kind = V2Widgets.ButtonKind.PRIMARY,
            onClick = {
                val raw = edit.text.toString().trim()
                if (raw.isEmpty()) {
                    scanErrorText = null
                    appMode = AppMode.STREAM
                    startStream(null, Protocol.DEFAULT_PORT, null)
                    navigateTo(Screen.MAIN)
                    return@actionButton
                }
                if (!raw.startsWith("motion://")) {
                    scanErrorText = "Expected a motion://host:port?pair=... pairing URI."
                    syncLiveState(forceRebuild = true)
                    return@actionButton
                }
                val parsed = runCatching { Protocol.parsePairingUri(raw) }.getOrNull()
                if (parsed == null) {
                    scanErrorText = "Invalid pairing URI. Check host, port, and pair secret."
                    syncLiveState(forceRebuild = true)
                    return@actionButton
                }
                scanErrorText = null
                prefs().edit().putString("last_uri", raw).apply()
                appMode = AppMode.STREAM
                startStream(parsed.host, parsed.port, parsed.pairSecret)
                navigateTo(Screen.MAIN)
            },
        ).apply {
            layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
        }
        val listenBtn = V2Widgets.actionButton(
            context = this,
            p = p,
            label = "Listen USB",
            kind = V2Widgets.ButtonKind.SECONDARY,
            iconRes = R.drawable.ic_lucide_plug,
            onClick = {
                scanErrorText = null
                appMode = AppMode.STREAM
                startStream(null, Protocol.DEFAULT_PORT, null)
                navigateTo(Screen.MAIN)
            },
        )
        actionRow.addView(connectBtn)
        actionRow.addView(V2Widgets.hSpace(this, 8f))
        actionRow.addView(listenBtn)
        body.addView(actionRow)
    }

    private fun rebuildPairDynamic(stream: StreamService.StreamState) {
        val p = palette
        val slot = pairDynamicSlot ?: return
        slot.removeAllViews()

        if (scanErrorText != null) {
            slot.addView(
                V2Widgets.noticeBox(
                    context = this,
                    p = p,
                    tone = V2Widgets.NoticeTone.ERR,
                    title = "Pairing scan failed",
                    body = scanErrorText!!,
                ),
            )
            slot.addView(V2Widgets.vSpace(this, 10f))
        } else if (stream is StreamService.StreamState.Rejected) {
            slot.addView(
                V2Widgets.noticeBox(
                    context = this,
                    p = p,
                    tone = V2Widgets.NoticeTone.ERR,
                    title = "Desktop rejected token",
                    body = "${stream.reason}. Scan a new QR code from the desktop app.",
                ),
            )
            slot.addView(V2Widgets.vSpace(this, 10f))
        }

        val pairedName = prefs().getString("paired_desktop", null)
        val hasToken = prefs().getString("paired_token", null) != null
        val card = V2Widgets.cardColumn(this, p, padV = 6f)
        card.addView(
            V2Widgets.infoRow(
                context = this,
                p = p,
                label = "Paired desktop",
                value = pairedName ?: "None",
            ),
        )
        if (hasToken || pairedName != null) {
            card.addView(V2Widgets.hairline(this, p))
            val btnRow = LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                setPadding(0, UiKit.dp(context, 8f), 0, UiKit.dp(context, 4f))
            }
            if (!confirmForget) {
                btnRow.addView(
                    V2Widgets.actionButton(
                        context = this,
                        p = p,
                        label = "Forget desktop",
                        kind = V2Widgets.ButtonKind.DANGER,
                        onClick = {
                            confirmForget = true
                            syncLiveState(forceRebuild = true)
                        },
                    ),
                )
            } else {
                btnRow.addView(
                    V2Widgets.actionButton(
                        context = this,
                        p = p,
                        label = "Confirm forget",
                        kind = V2Widgets.ButtonKind.DANGER,
                        onClick = {
                            prefs().edit()
                                .remove("paired_token")
                                .remove("paired_desktop")
                                .apply()
                            confirmForget = false
                            syncLiveState(forceRebuild = true)
                        },
                    ),
                )
                btnRow.addView(V2Widgets.hSpace(this, 8f))
                btnRow.addView(
                    V2Widgets.actionButton(
                        context = this,
                        p = p,
                        label = "Keep",
                        kind = V2Widgets.ButtonKind.SECONDARY,
                        onClick = {
                            confirmForget = false
                            syncLiveState(forceRebuild = true)
                        },
                    ),
                )
            }
            card.addView(btnRow)
        }
        slot.addView(card)
        slot.addView(V2Widgets.vSpace(this, 12f))
    }

    private fun buildPermission(body: LinearLayout, p: V2Palette) {
        val granted = Settings.canDrawOverlays(this)
        val statusCard = V2Widgets.cardColumn(this, p, padV = 6f)
        statusCard.addView(
            V2Widgets.infoRow(
                context = this,
                p = p,
                label = "Display over apps",
                value = if (granted) "Granted" else "Required",
                valueColor = if (granted) p.live else p.warn,
            ),
        )
        body.addView(statusCard)
        body.addView(V2Widgets.vSpace(this, 12f))

        val stepsCard = V2Widgets.cardColumn(this, p)
        stepsCard.addView(
            V2Widgets.bodyText(
                context = this,
                p = p,
                text = "1. Open Android overlay settings below.\n2. Find KineAnchor and enable Allow display over other apps.\n3. Return here to start the motion cues.",
                color = p.ink,
                sizeSp = 13f,
            ),
        )
        body.addView(stepsCard)
        body.addView(V2Widgets.vSpace(this, 12f))

        body.addView(
            V2Widgets.actionButton(
                context = this,
                p = p,
                label = if (granted) "Open system settings" else "Allow display over apps",
                kind = V2Widgets.ButtonKind.PRIMARY,
                onClick = {
                    val intent = Intent(
                        Settings.ACTION_MANAGE_OVERLAY_PERMISSION,
                        Uri.parse("package:$packageName"),
                    )
                    startActivity(intent)
                },
            ),
        )
        body.addView(V2Widgets.vSpace(this, 10f))
        body.addView(
            V2Widgets.bodyText(
                context = this,
                p = p,
                text = "Android limits overlay windows to 0.80 alpha so touches always pass through to apps underneath.",
                color = p.inkMuted,
                sizeSp = 12f,
            ),
        )
    }

    private fun buildSettings(body: LinearLayout, p: V2Palette) {
        body.addView(
            V2Widgets.segmented(
                context = this,
                p = p,
                options = listOf("Preferences", "Privacy", "Licenses"),
                selected = settingsTab,
                onSelect = { idx ->
                    settingsTab = idx
                    confirmForget = false
                    renderScreen()
                },
            ),
        )
        body.addView(V2Widgets.vSpace(this, 14f))

        when (settingsTab) {
            0 -> buildPreferencesTab(body, p)
            1 -> buildPrivacyTab(body, p)
            else -> buildLicensesTab(body, p)
        }
    }

    private fun buildPreferencesTab(body: LinearLayout, p: V2Palette) {
        body.addView(V2Widgets.sectionCaption(this, p, "Interface"))
        val uiCard = V2Widgets.cardColumn(this, p)
        uiCard.addView(
            V2Widgets.bodyText(
                context = this,
                p = p,
                text = "Show advanced info",
                color = p.ink,
                sizeSp = 14f,
            ),
        )
        uiCard.addView(V2Widgets.vSpace(this, 4f))
        uiCard.addView(
            V2Widgets.bodyText(
                context = this,
                p = p,
                text = "Use literal technical names (Gain, Opacity, 6-DoF) and show raw Hz, offset, and transport telemetry.",
                color = p.inkSecondary,
                sizeSp = 12.5f,
            ),
        )
        uiCard.addView(V2Widgets.vSpace(this, 10f))
        uiCard.addView(
            V2Widgets.segmented(
                context = this,
                p = p,
                options = listOf("Off", "On"),
                selected = if (showAdvancedInfo) 1 else 0,
                onSelect = { idx ->
                    val next = idx == 1
                    if (next != showAdvancedInfo) {
                        showAdvancedInfo = next
                        saveConfig()
                        renderScreen()
                    }
                },
            ),
        )
        body.addView(uiCard)

        body.addView(V2Widgets.vSpace(this, 14f))
        body.addView(V2Widgets.sectionCaption(this, p, "Calibration & filter"))
        val calCard = V2Widgets.cardColumn(this, p)
        calCard.addView(
            V2Widgets.bodyText(
                context = this,
                p = p,
                text = if (showAdvancedInfo) {
                    "Spring ODE: omega 6.0 rad/s, zeta 0.70, tau 0.50 s · 500 ms watchdog"
                } else {
                    "Motion smoothing damps jerky bumps and returns cues gently to rest."
                },
                color = p.inkSecondary,
                sizeSp = 12.5f,
            ),
        )
        calCard.addView(V2Widgets.vSpace(this, 10f))
        calCard.addView(
            V2Widgets.actionButton(
                context = this,
                p = p,
                label = "Reset defaults",
                kind = V2Widgets.ButtonKind.SECONDARY,
                onClick = {
                    resetDefaults()
                    renderScreen()
                },
            ),
        )
        body.addView(calCard)

        body.addView(V2Widgets.vSpace(this, 14f))
        body.addView(V2Widgets.sectionCaption(this, p, "Paired desktop"))
        val pairCard = V2Widgets.cardColumn(this, p, padV = 6f)
        val pairedName = prefs().getString("paired_desktop", null)
        val hasToken = prefs().getString("paired_token", null) != null
        pairCard.addView(V2Widgets.infoRow(this, p, "Desktop", pairedName ?: "None"))
        if (hasToken || pairedName != null) {
            pairCard.addView(V2Widgets.hairline(this, p))
            val btnRow = LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                setPadding(0, UiKit.dp(context, 8f), 0, UiKit.dp(context, 4f))
            }
            if (!confirmForget) {
                btnRow.addView(
                    V2Widgets.actionButton(
                        context = this,
                        p = p,
                        label = "Revoke token",
                        kind = V2Widgets.ButtonKind.DANGER,
                        onClick = {
                            confirmForget = true
                            renderScreen()
                        },
                    ),
                )
            } else {
                btnRow.addView(
                    V2Widgets.actionButton(
                        context = this,
                        p = p,
                        label = "Confirm revoke",
                        kind = V2Widgets.ButtonKind.DANGER,
                        onClick = {
                            prefs().edit()
                                .remove("paired_token")
                                .remove("paired_desktop")
                                .apply()
                            confirmForget = false
                            renderScreen()
                        },
                    ),
                )
                btnRow.addView(V2Widgets.hSpace(this, 8f))
                btnRow.addView(
                    V2Widgets.actionButton(
                        context = this,
                        p = p,
                        label = "Cancel",
                        kind = V2Widgets.ButtonKind.SECONDARY,
                        onClick = {
                            confirmForget = false
                            renderScreen()
                        },
                    ),
                )
            }
            pairCard.addView(btnRow)
        }
        body.addView(pairCard)
    }

    private fun buildPrivacyTab(body: LinearLayout, p: V2Palette) {
        body.addView(V2Widgets.sectionCaption(this, p, "Application"))
        val appCard = V2Widgets.cardColumn(this, p, padV = 4f)
        appCard.addView(V2Widgets.infoRow(this, p, "Version", "0.1.1", monoValue = true))
        appCard.addView(V2Widgets.hairline(this, p))
        appCard.addView(V2Widgets.infoRow(this, p, "License", "GPL-3.0-or-later"))
        appCard.addView(V2Widgets.hairline(this, p))
        val granted = Settings.canDrawOverlays(this)
        appCard.addView(
            V2Widgets.infoRow(
                this,
                p,
                "Display over apps",
                if (granted) "Granted" else "Required",
                valueColor = if (granted) p.live else p.warn,
            ),
        )
        body.addView(appCard)

        body.addView(V2Widgets.vSpace(this, 14f))
        body.addView(V2Widgets.sectionCaption(this, p, "Zero telemetry · Offline"))
        val privacyCard = V2Widgets.cardColumn(this, p)
        privacyCard.addView(
            V2Widgets.bodyText(
                context = this,
                p = p,
                text = "Sensor telemetry stays on your device or streams directly to your paired computer over local Wi-Fi or USB. KineAnchor makes no analytics or cloud requests.",
                color = p.inkSecondary,
                sizeSp = 13f,
            ),
        )
        body.addView(privacyCard)
    }

    private fun buildLicensesTab(body: LinearLayout, p: V2Palette) {
        body.addView(V2Widgets.sectionCaption(this, p, "GNU GPL v3.0 or later"))
        val licenseCard = V2Widgets.cardColumn(this, p)
        licenseCard.addView(
            V2Widgets.bodyText(
                context = this,
                p = p,
                text = "Kotlin shell + Rust core via zero-dep C-ABI JNI (libkineanchor_android_ffi.so).",
                color = p.inkSecondary,
                sizeSp = 12.5f,
            ),
        )
        body.addView(licenseCard)

        body.addView(V2Widgets.vSpace(this, 14f))
        body.addView(V2Widgets.sectionCaption(this, p, "Open-source dependencies"))
        val depCard = V2Widgets.cardColumn(this, p, padV = 4f)
        val deps = listOf(
            "KineAnchor Core (Rust)" to "GPL-3.0-or-later",
            "AndroidX AppCompat & CameraX" to "Apache-2.0",
            "ZXing Core" to "Apache-2.0",
            "Lucide Icons" to "ISC",
        )
        deps.forEachIndexed { idx, (name, lic) ->
            if (idx > 0) depCard.addView(V2Widgets.hairline(this, p))
            depCard.addView(V2Widgets.infoRow(this, p, name, lic, monoValue = true, valueColor = p.inkSecondary))
        }
        body.addView(depCard)

        body.addView(V2Widgets.vSpace(this, 14f))
        body.addView(V2Widgets.sectionCaption(this, p, "Research & prior art"))
        val researchCard = V2Widgets.cardColumn(this, p, padV = 4f)
        researchCard.addView(V2Widgets.infoRow(this, p, "Diels & Howarth (2013)", "Human Factors"))
        researchCard.addView(V2Widgets.hairline(this, p))
        researchCard.addView(V2Widgets.infoRow(this, p, "Clean-room prior art", "0 lines copied"))
        body.addView(researchCard)
    }

    private fun strengthLabel(v: Float): String =
        when {
            v < 0.85f -> "Gentle"
            v < 1.3f -> "Balanced"
            v < 2.0f -> "Strong"
            else -> "Max"
        }

    private fun sizeLabel(dp: Float): String =
        when {
            dp < 8.0f -> "Small"
            dp < 13.0f -> "Medium"
            else -> "Large"
        }

    private fun densityLabel(d: Float): String =
        when {
            d < 0.85f -> "Few"
            d < 1.35f -> "Normal"
            else -> "Many"
        }

    private fun visibilityLabel(alpha: Float): String =
        when {
            alpha < 0.35f -> "Gentle"
            alpha < 0.55f -> "Light"
            alpha < 0.72f -> "Medium"
            else -> "Strong"
        }

    private fun startOverlayService() {
        val intent = Intent(this, OverlayService::class.java).apply {
            action = OverlayService.ACTION_START
            putExtra(OverlayService.EXTRA_VEHICLE, selectedVehicle)
            putExtra(OverlayService.EXTRA_SOURCE, selectedSource)
            putExtra(OverlayService.EXTRA_DENSITY, density)
            putExtra(OverlayService.EXTRA_RADIUS, dotRadius)
            putExtra(OverlayService.EXTRA_OPACITY, opacity.coerceAtMost(0.8f))
            putExtra(OverlayService.EXTRA_GAIN, gain)
        }
        startForegroundService(intent)
    }

    private fun startStream(host: String?, port: Int, pairSecret: String?) {
        val intent = Intent(this, StreamService::class.java).apply {
            action = StreamService.ACTION_START
            if (host != null) putExtra(StreamService.EXTRA_HOST, host)
            putExtra(StreamService.EXTRA_PORT, port)
            if (pairSecret != null) putExtra(StreamService.EXTRA_PAIR_SECRET, pairSecret)
        }
        startForegroundService(intent)
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
            renderScreen()
            return
        }
        if (!packageManager.hasSystemFeature(PackageManager.FEATURE_CAMERA_ANY)) {
            scanErrorText = "No camera hardware on this device. Enter the motion:// URI below."
            syncLiveState(forceRebuild = true)
            return
        }
        val granted = ContextCompat.checkSelfPermission(
            this,
            Manifest.permission.CAMERA,
        ) == PackageManager.PERMISSION_GRANTED
        if (granted) {
            scanErrorText = null
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
                scanErrorText = "Camera service unavailable. Enter the motion:// URI below."
                syncLiveState(forceRebuild = true)
                return@addListener
            }
            val hasCamera = runCatching {
                provider.hasCamera(CameraSelector.DEFAULT_BACK_CAMERA) ||
                    provider.hasCamera(CameraSelector.DEFAULT_FRONT_CAMERA)
            }.getOrDefault(false)
            if (!hasCamera) {
                scanErrorText = "No usable camera found. Enter the motion:// URI below."
                provider.unbindAll()
                syncLiveState(forceRebuild = true)
                return@addListener
            }

            cameraProvider = provider
            scannedPairingHandled = false
            scannerActive = true
            renderScreen()

            val pv = previewView ?: return@addListener
            val preview = Preview.Builder().build().also {
                it.surfaceProvider = pv.surfaceProvider
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
                scanErrorText = "Camera failed to start (${error.message ?: "unknown"})."
                stopQrScan()
                renderScreen()
                return@addListener
            }

            camera.cameraInfo.cameraState.observe(this) { camState ->
                if (camState.type == CameraState.Type.CLOSED && camState.error != null) {
                    scanErrorText = "Camera closed with error ${camState.error!!.code}."
                    stopQrScan()
                    renderScreen()
                }
            }
        }, ContextCompat.getMainExecutor(this))
    }

    private fun stopQrScan() {
        scannerActive = false
        previewView?.visibility = View.GONE
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
                    scanErrorText = "QR code is not a KineAnchor motion:// pairing code."
                    syncLiveState(forceRebuild = true)
                }
                return
            }
            val parsed = runCatching { Protocol.parsePairingUri(uri) }.getOrNull()
            if (parsed == null) {
                runOnUiThread {
                    scanErrorText = "QR code is missing host, port, or pair secret."
                    syncLiveState(forceRebuild = true)
                }
                return
            }
            scannedPairingHandled = true
            runOnUiThread {
                scanErrorText = null
                uriInput?.setText(uri)
                prefs().edit().putString("last_uri", uri).apply()
                stopQrScan()
                appMode = AppMode.STREAM
                startStream(parsed.host, parsed.port, parsed.pairSecret)
                navigateTo(Screen.MAIN)
            }
        } finally {
            image.close()
        }
    }

    private fun probeUsbState(): Pair<String, Boolean> {
        val sticky = registerReceiver(null, IntentFilter("android.hardware.usb.action.USB_STATE"))
        val connected = sticky?.getBooleanExtra("connected", false) ?: false
        val rndis = sticky?.getBooleanExtra("rndis", false) ?: false
        val ncm = sticky?.getBooleanExtra("ncm", false) ?: false
        return when {
            rndis || ncm -> "Tether active" to true
            connected -> "Connected (tether off)" to false
            else -> "No USB data cable" to false
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
