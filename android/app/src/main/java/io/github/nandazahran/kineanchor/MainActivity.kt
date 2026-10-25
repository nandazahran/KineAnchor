// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.nandazahran.kineanchor

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.Bundle
import android.provider.Settings
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity
import com.google.zxing.BinaryBitmap
import com.google.zxing.MultiFormatReader
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.common.HybridBinarizer

class MainActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val prefs = getSharedPreferences("kineanchor_prefs", Context.MODE_PRIVATE)
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(48, 48, 48, 48)
        }

        val title = TextView(this).apply {
            text = "KineAnchor — Phone Sensor Puck"
            textSize = 20f
        }
        val usbStatus = TextView(this).apply {
            text = detectUsbStateText()
        }
        val tetherBtn = Button(this).apply {
            text = "1. Open USB Tethering Settings"
            setOnClickListener { openTetherSettings() }
        }
        val uriInput = EditText(this).apply {
            hint = "motion://192.168.x.x:47474?pair=... (or scan QR)"
            setText(prefs.getString("last_uri", ""))
        }
        val startBtn = Button(this).apply {
            text = "2. Pair & Start 50Hz Sensor Stream"
            setOnClickListener {
                val raw = uriInput.text.toString().trim()
                val intent = Intent(this@MainActivity, StreamService::class.java).apply {
                    action = StreamService.ACTION_START
                    if (raw.startsWith("motion://")) {
                        runCatching { Protocol.parsePairingUri(raw) }.onSuccess { p ->
                            putExtra(StreamService.EXTRA_HOST, p.host)
                            putExtra(StreamService.EXTRA_PORT, p.port)
                            putExtra(StreamService.EXTRA_PAIR_SECRET, p.pairSecret)
                            prefs.edit().putString("last_uri", raw).apply()
                        }
                    }
                }
                startForegroundService(intent)
                usbStatus.text = "Streaming active (50Hz)"
            }
        }
        val stopBtn = Button(this).apply {
            text = "Stop Stream"
            setOnClickListener {
                val intent = Intent(this@MainActivity, StreamService::class.java).apply {
                    action = StreamService.ACTION_STOP
                }
                startService(intent)
                usbStatus.text = "Stream stopped"
            }
        }

        root.addView(title)
        root.addView(usbStatus)
        root.addView(tetherBtn)
        root.addView(uriInput)
        root.addView(startBtn)
        root.addView(stopBtn)
        setContentView(root)
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
        /** Offline QR decode helper using ZXing on a YUV420 frame from CameraX ImageAnalysis. */
        fun decodeQrFromYuv(
            yPlane: ByteArray,
            width: Int,
            height: Int,
        ): String? {
            val source = PlanarYUVLuminanceSource(yPlane, width, height, 0, 0, width, height, false)
            val bitmap = BinaryBitmap(HybridBinarizer(source))
            return runCatching { MultiFormatReader().decode(bitmap).text }.getOrNull()
        }
    }
}
