// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.nandazahran.kineanchor

import android.content.Context
import android.content.res.Configuration
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.util.TypedValue

/**
 * Precision Instrument v2 tokens matching Doop (`KineAnchor` / `-5ZagVyCuf`) and desktop `design.rs`.
 */
data class V2Palette(
    val bg: Int,
    val surface: Int,
    val well: Int,
    val border: Int,
    val ink: Int,
    val inkSecondary: Int,
    val inkMuted: Int,
    val ctaBg: Int,
    val ctaInk: Int,
    val accent: Int,
    val live: Int,
    val segActive: Int,
    val warn: Int,
    val warnBg: Int,
    val warnBorder: Int,
    val err: Int,
    val errBg: Int,
    val errBorder: Int,
) {
    companion object {
        val Dark = V2Palette(
            bg = Color.parseColor("#111214"),
            surface = Color.parseColor("#17181B"),
            well = Color.parseColor("#1D1F23"),
            border = Color.parseColor("#27292E"),
            ink = Color.parseColor("#F2F3F5"),
            inkSecondary = Color.parseColor("#9BA0A8"),
            inkMuted = Color.parseColor("#686D75"),
            ctaBg = Color.parseColor("#F2F3F5"),
            ctaInk = Color.parseColor("#111214"),
            accent = Color.parseColor("#38BDF8"),
            live = Color.parseColor("#22C55E"),
            segActive = Color.parseColor("#25282E"),
            warn = Color.parseColor("#F59E0B"),
            warnBg = Color.parseColor("#1C1810"),
            warnBorder = Color.parseColor("#3E3219"),
            err = Color.parseColor("#F87171"),
            errBg = Color.parseColor("#1F1416"),
            errBorder = Color.parseColor("#452226"),
        )

        val Light = V2Palette(
            bg = Color.parseColor("#F7F7F5"),
            surface = Color.parseColor("#FFFFFF"),
            well = Color.parseColor("#EEEEEB"),
            border = Color.parseColor("#E2E2DE"),
            ink = Color.parseColor("#141517"),
            inkSecondary = Color.parseColor("#5C5E63"),
            inkMuted = Color.parseColor("#8A8D93"),
            ctaBg = Color.parseColor("#141517"),
            ctaInk = Color.parseColor("#FFFFFF"),
            accent = Color.parseColor("#0284C7"),
            live = Color.parseColor("#16A34A"),
            segActive = Color.parseColor("#FFFFFF"),
            warn = Color.parseColor("#B45309"),
            warnBg = Color.parseColor("#FEF9EC"),
            warnBorder = Color.parseColor("#F0D9A0"),
            err = Color.parseColor("#B91C1C"),
            errBg = Color.parseColor("#FEF2F2"),
            errBorder = Color.parseColor("#F5C2C2"),
        )

        fun forContext(context: Context): V2Palette {
            val night = context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK
            return if (night == Configuration.UI_MODE_NIGHT_YES) Dark else Light
        }
    }
}

object UiKit {
    fun dp(context: Context, value: Float): Int =
        TypedValue.applyDimension(
            TypedValue.COMPLEX_UNIT_DIP,
            value,
            context.resources.displayMetrics,
        ).toInt()

    fun dpF(context: Context, value: Float): Float =
        TypedValue.applyDimension(
            TypedValue.COMPLEX_UNIT_DIP,
            value,
            context.resources.displayMetrics,
        )

    fun boxDrawable(
        context: Context,
        fill: Int,
        stroke: Int? = null,
        radiusDp: Float = 6f,
    ): GradientDrawable =
        GradientDrawable().apply {
            shape = GradientDrawable.RECTANGLE
            cornerRadius = dpF(context, radiusDp)
            setColor(fill)
            if (stroke != null) {
                setStroke(dp(context, 1f).coerceAtLeast(1), stroke)
            }
        }

    fun dotDrawable(color: Int): GradientDrawable =
        GradientDrawable().apply {
            shape = GradientDrawable.OVAL
            setColor(color)
        }

    fun sansMedium(): Typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)

    fun mono(): Typeface = Typeface.MONOSPACE
}
