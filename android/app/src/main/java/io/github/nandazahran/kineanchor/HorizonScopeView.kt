// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.nandazahran.kineanchor

import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.RectF
import android.view.View
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.sin

/**
 * Architectural `8dp` horizon telemetry & peripheral cue preview (`V2Scope`).
 */
class HorizonScopeView(
    context: Context,
    private var palette: V2Palette,
) : View(context) {
    enum class ScopeTone {
        LIVE,
        WARN,
        ERR,
        MUTED,
    }

    private var statusText: String = "Idle"
    private var statusTone: ScopeTone = ScopeTone.MUTED
    private var readoutText: String = "0.00 / 0.00"
    private var offsetX: Float = 0f
    private var offsetY: Float = 0f
    private var syntheticMode: Boolean = false
    private var activeMotion: Boolean = false
    private var phase: Float = 0f

    private val rect = RectF()
    private val bgPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.FILL }
    private val borderPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.STROKE }
    private val gridPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.STROKE }
    private val cuePaint = Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.FILL }
    private val horizonPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.STROKE }
    private val dotPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.FILL }
    private val labelPaint = Paint(Paint.ANTI_ALIAS_FLAG)
    private val monoPaint = Paint(Paint.ANTI_ALIAS_FLAG)

    init {
        isClickable = false
        isFocusable = false
    }

    fun updatePalette(next: V2Palette) {
        palette = next
        invalidate()
    }

    fun updateState(
        label: String,
        tone: ScopeTone,
        horizonX: Float,
        horizonY: Float,
        synthetic: Boolean,
        active: Boolean,
    ) {
        statusText = label
        statusTone = tone
        offsetX = horizonX.coerceIn(-1f, 1f)
        offsetY = horizonY.coerceIn(-1f, 1f)
        syntheticMode = synthetic
        activeMotion = active
        readoutText = String.format("%+.2f / %+.2f", offsetX, offsetY)
        invalidate()
    }

    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        val desiredW = MeasureSpec.getSize(widthMeasureSpec)
        val desiredH = UiKit.dp(context, 136f)
        setMeasuredDimension(desiredW, resolveSize(desiredH, heightMeasureSpec))
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        val w = width.toFloat()
        val h = height.toFloat()
        if (w <= 0f || h <= 0f) return

        val strokePx = UiKit.dpF(context, 1f)
        val radiusPx = UiKit.dpF(context, 8f)
        rect.set(strokePx * 0.5f, strokePx * 0.5f, w - strokePx * 0.5f, h - strokePx * 0.5f)

        bgPaint.color = palette.well
        canvas.drawRoundRect(rect, radiusPx, radiusPx, bgPaint)

        // Subtle 40dp architectural grid clipped inside scope bounds.
        canvas.save()
        val clipPath = android.graphics.Path().apply {
            addRoundRect(rect, radiusPx, radiusPx, android.graphics.Path.Direction.CW)
        }
        canvas.clipPath(clipPath)

        gridPaint.color = palette.border
        gridPaint.strokeWidth = strokePx
        val step = UiKit.dpF(context, 40f)
        var x = step
        while (x < w) {
            canvas.drawLine(x, 0f, x, h, gridPaint)
            x += step
        }
        var y = step
        while (y < h) {
            canvas.drawLine(0f, y, w, y, gridPaint)
            y += step
        }

        val cx = w * 0.5f
        val cy = h * 0.56f
        val crossHalf = UiKit.dpF(context, 10f)
        canvas.drawLine(cx - crossHalf, cy, cx + crossHalf, cy, gridPaint)
        canvas.drawLine(cx, cy - crossHalf, cx, cy + crossHalf, gridPaint)

        // Horizon cue dots (animated smoothly when synthetic demo is active).
        val dx: Float
        val dy: Float
        if (syntheticMode && activeMotion) {
            phase += 0.04f
            if (phase > (2f * PI).toFloat()) phase -= (2f * PI).toFloat()
            dx = cos(phase) * 0.35f
            dy = sin(phase * 2f) * 0.14f
        } else {
            dx = offsetX
            dy = offsetY
        }

        val cueShiftX = dx * UiKit.dpF(context, 28f)
        val cueShiftY = dy * UiKit.dpF(context, 18f)

        val isDimmed = statusTone == ScopeTone.MUTED || statusTone == ScopeTone.ERR
        horizonPaint.color = palette.accent
        horizonPaint.alpha = if (isDimmed) 70 else 190
        horizonPaint.strokeWidth = UiKit.dpF(context, 1.5f)
        val lineHalf = UiKit.dpF(context, 34f)
        canvas.drawLine(
            cx - lineHalf + cueShiftX,
            cy + cueShiftY,
            cx + lineHalf + cueShiftX,
            cy + cueShiftY,
            horizonPaint,
        )

        cuePaint.color = palette.accent
        cuePaint.alpha = if (isDimmed) 64 else 210
        val dotRadius = UiKit.dpF(context, 3.5f)
        val ringR = UiKit.dpF(context, 44f)
        for (i in 0 until 8) {
            val angle = (i.toFloat() / 8f) * (2f * PI).toFloat()
            val px = cx + cos(angle) * ringR * 1.45f + cueShiftX
            val py = cy + sin(angle) * ringR * 0.65f + cueShiftY
            canvas.drawCircle(px, py, dotRadius, cuePaint)
        }

        canvas.restore()

        borderPaint.color = palette.border
        borderPaint.strokeWidth = strokePx
        canvas.drawRoundRect(rect, radiusPx, radiusPx, borderPaint)

        // Top-left status dot + label, top-right monospace coordinate readout.
        val padX = UiKit.dpF(context, 12f)
        val padY = UiKit.dpF(context, 16f)
        val statusColor = when (statusTone) {
            ScopeTone.LIVE -> palette.live
            ScopeTone.WARN -> palette.warn
            ScopeTone.ERR -> palette.err
            ScopeTone.MUTED -> palette.inkMuted
        }
        dotPaint.color = statusColor
        canvas.drawCircle(padX + UiKit.dpF(context, 3f), padY - UiKit.dpF(context, 3.5f), UiKit.dpF(context, 3f), dotPaint)

        labelPaint.color = palette.ink
        labelPaint.typeface = UiKit.sansMedium()
        labelPaint.textSize = UiKit.dpF(context, 12f)
        labelPaint.textAlign = Paint.Align.LEFT
        canvas.drawText(statusText, padX + UiKit.dpF(context, 12f), padY, labelPaint)

        monoPaint.color = palette.inkSecondary
        monoPaint.typeface = UiKit.mono()
        monoPaint.textSize = UiKit.dpF(context, 11f)
        monoPaint.textAlign = Paint.Align.RIGHT
        canvas.drawText(readoutText, w - padX, padY, monoPaint)

        if (activeMotion) {
            postInvalidateOnAnimation()
        }
    }
}
