// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.nandazahran.kineanchor

import android.content.Context
import android.content.res.ColorStateList
import android.graphics.drawable.ClipDrawable
import android.graphics.drawable.Drawable
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.LayerDrawable
import android.text.TextUtils
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.SeekBar
import android.widget.TextView
import androidx.annotation.DrawableRes
import androidx.core.content.ContextCompat

/**
 * Reusable Precision Instrument v2 UI primitives matching Doop `-5ZagVyCuf`.
 */
object V2Widgets {
    enum class ButtonKind {
        PRIMARY,
        SECONDARY,
        DANGER,
    }

    enum class NoticeTone {
        WARN,
        ERR,
    }

    fun vSpace(context: Context, dp: Float): View =
        View(context).apply {
            layoutParams = LinearLayout.LayoutParams(1, UiKit.dp(context, dp))
        }

    fun hSpace(context: Context, dp: Float): View =
        View(context).apply {
            layoutParams = LinearLayout.LayoutParams(UiKit.dp(context, dp), 1)
        }

    fun hairline(context: Context, p: V2Palette): View =
        View(context).apply {
            setBackgroundColor(p.border)
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                UiKit.dp(context, 1f).coerceAtLeast(1),
            )
        }

    fun sectionCaption(context: Context, p: V2Palette, text: String): TextView =
        TextView(context).apply {
            this.text = text
            setTextColor(p.inkMuted)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 12f)
            typeface = UiKit.sansMedium()
            setPadding(0, UiKit.dp(context, 4f), 0, UiKit.dp(context, 6f))
        }

    fun bodyText(
        context: Context,
        p: V2Palette,
        text: String,
        color: Int = p.inkSecondary,
        sizeSp: Float = 13f,
    ): TextView =
        TextView(context).apply {
            this.text = text
            setTextColor(color)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, sizeSp)
            setLineSpacing(UiKit.dpF(context, 2f), 1.0f)
        }

    fun cardColumn(
        context: Context,
        p: V2Palette,
        padH: Float = 14f,
        padV: Float = 12f,
    ): LinearLayout =
        LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            background = UiKit.boxDrawable(context, p.surface, p.border, 6f)
            setPadding(
                UiKit.dp(context, padH),
                UiKit.dp(context, padV),
                UiKit.dp(context, padH),
                UiKit.dp(context, padV),
            )
        }

    fun tintedIcon(
        context: Context,
        @DrawableRes iconRes: Int,
        tint: Int,
        sizeDp: Float = 18f,
    ): ImageView =
        ImageView(context).apply {
            val px = UiKit.dp(context, sizeDp)
            layoutParams = LinearLayout.LayoutParams(px, px)
            val drawable: Drawable? = ContextCompat.getDrawable(context, iconRes)?.mutate()
            setImageDrawable(drawable)
            imageTintList = ColorStateList.valueOf(tint)
        }

    fun topHeader(
        context: Context,
        p: V2Palette,
        title: String,
        leadingIcon: Int? = null,
        onBack: (() -> Unit)? = null,
        trailingActions: List<Pair<Int, Pair<String, () -> Unit>>> = emptyList(),
    ): LinearLayout =
        LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(
                UiKit.dp(context, 16f),
                UiKit.dp(context, 12f),
                UiKit.dp(context, 16f),
                UiKit.dp(context, 12f),
            )

            if (onBack != null) {
                val backBtn = iconButton(context, p, R.drawable.ic_lucide_chevron_left, "Back", onBack)
                addView(backBtn)
                addView(hSpace(context, 10f))
            } else if (leadingIcon != null) {
                addView(tintedIcon(context, leadingIcon, p.accent, 20f))
                addView(hSpace(context, 8f))
            }

            val titleView = TextView(context).apply {
                text = title
                setTextColor(p.ink)
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 16f)
                typeface = UiKit.sansMedium()
                layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
            }
            addView(titleView)

            trailingActions.forEachIndexed { idx, (iconRes, pair) ->
                if (idx > 0) addView(hSpace(context, 8f))
                addView(iconButton(context, p, iconRes, pair.first, pair.second))
            }
        }

    fun iconButton(
        context: Context,
        p: V2Palette,
        @DrawableRes iconRes: Int,
        desc: String,
        onClick: () -> Unit,
    ): LinearLayout =
        LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER
            contentDescription = desc
            val size = UiKit.dp(context, 36f)
            layoutParams = LinearLayout.LayoutParams(size, size)
            background = UiKit.boxDrawable(context, p.surface, p.border, 6f)
            addView(tintedIcon(context, iconRes, p.ink, 17f))
            setOnClickListener { onClick() }
        }

    fun actionButton(
        context: Context,
        p: V2Palette,
        label: String,
        kind: ButtonKind,
        @DrawableRes iconRes: Int? = null,
        enabled: Boolean = true,
        onClick: () -> Unit,
    ): LinearLayout {
        val (fg, bg, stroke) = when (kind) {
            ButtonKind.PRIMARY -> Triple(p.ctaInk, p.ctaBg, p.ctaBg)
            ButtonKind.SECONDARY -> Triple(p.ink, p.surface, p.border)
            ButtonKind.DANGER -> Triple(p.err, p.errBg, p.errBorder)
        }
        return LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER
            isEnabled = enabled
            alpha = if (enabled) 1.0f else 0.45f
            background = UiKit.boxDrawable(context, bg, stroke, 6f)
            setPadding(
                UiKit.dp(context, 14f),
                UiKit.dp(context, 11f),
                UiKit.dp(context, 14f),
                UiKit.dp(context, 11f),
            )
            if (iconRes != null) {
                addView(tintedIcon(context, iconRes, fg, 16f))
                addView(hSpace(context, 7f))
            }
            addView(
                TextView(context).apply {
                    text = label
                    setTextColor(fg)
                    setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
                    typeface = UiKit.sansMedium()
                    maxLines = 1
                    ellipsize = TextUtils.TruncateAt.END
                },
            )
            if (enabled) {
                setOnClickListener { onClick() }
            }
        }
    }

    fun segmented(
        context: Context,
        p: V2Palette,
        options: List<String>,
        selected: Int,
        onSelect: (Int) -> Unit,
    ): LinearLayout =
        LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            background = UiKit.boxDrawable(context, p.well, p.border, 6f)
            val pad = UiKit.dp(context, 3f)
            setPadding(pad, pad, pad, pad)

            options.forEachIndexed { idx, label ->
                val active = idx == selected
                val item = TextView(context).apply {
                    text = label
                    gravity = Gravity.CENTER
                    setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
                    typeface = UiKit.sansMedium()
                    setTextColor(if (active) p.ink else p.inkSecondary)
                    background = if (active) UiKit.boxDrawable(context, p.segActive, null, 4f) else null
                    setPadding(
                        UiKit.dp(context, 8f),
                        UiKit.dp(context, 8f),
                        UiKit.dp(context, 8f),
                        UiKit.dp(context, 8f),
                    )
                    layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
                    setOnClickListener { onSelect(idx) }
                }
                addView(item)
            }
        }

    fun infoRow(
        context: Context,
        p: V2Palette,
        label: String,
        value: String,
        monoValue: Boolean = false,
        valueColor: Int = p.ink,
    ): LinearLayout =
        LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, UiKit.dp(context, 9f), 0, UiKit.dp(context, 9f))

            addView(
                TextView(context).apply {
                    text = label
                    setTextColor(p.inkSecondary)
                    setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
                    layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
                },
            )
            addView(
                TextView(context).apply {
                    text = value
                    setTextColor(valueColor)
                    setTextSize(TypedValue.COMPLEX_UNIT_SP, if (monoValue) 12f else 13f)
                    typeface = if (monoValue) UiKit.mono() else UiKit.sansMedium()
                },
            )
        }

    fun navRow(
        context: Context,
        p: V2Palette,
        title: String,
        subtitle: String? = null,
        onClick: () -> Unit,
    ): LinearLayout =
        LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            background = UiKit.boxDrawable(context, p.surface, p.border, 6f)
            setPadding(
                UiKit.dp(context, 14f),
                UiKit.dp(context, 12f),
                UiKit.dp(context, 12f),
                UiKit.dp(context, 12f),
            )
            val textCol = LinearLayout(context).apply {
                orientation = LinearLayout.VERTICAL
                layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
                addView(
                    TextView(context).apply {
                        text = title
                        setTextColor(p.ink)
                        setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
                        typeface = UiKit.sansMedium()
                    },
                )
                if (subtitle != null) {
                    addView(vSpace(context, 2f))
                    addView(
                        TextView(context).apply {
                            text = subtitle
                            setTextColor(p.inkSecondary)
                            setTextSize(TypedValue.COMPLEX_UNIT_SP, 12f)
                        },
                    )
                }
            }
            addView(textCol)
            addView(tintedIcon(context, R.drawable.ic_lucide_chevron_right, p.inkMuted, 16f))
            setOnClickListener { onClick() }
        }

    fun noticeBox(
        context: Context,
        p: V2Palette,
        tone: NoticeTone,
        title: String,
        body: String,
        @DrawableRes iconRes: Int = R.drawable.ic_lucide_alert_triangle,
        actionLabel: String? = null,
        onAction: (() -> Unit)? = null,
    ): LinearLayout {
        val (fg, bg, border) = when (tone) {
            NoticeTone.WARN -> Triple(p.warn, p.warnBg, p.warnBorder)
            NoticeTone.ERR -> Triple(p.err, p.errBg, p.errBorder)
        }
        return LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            background = UiKit.boxDrawable(context, bg, border, 6f)
            setPadding(
                UiKit.dp(context, 14f),
                UiKit.dp(context, 12f),
                UiKit.dp(context, 14f),
                UiKit.dp(context, 12f),
            )

            val header = LinearLayout(context).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                addView(tintedIcon(context, iconRes, fg, 16f))
                addView(hSpace(context, 8f))
                addView(
                    TextView(context).apply {
                        text = title
                        setTextColor(fg)
                        setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
                        typeface = UiKit.sansMedium()
                    },
                )
            }
            addView(header)
            addView(vSpace(context, 4f))
            addView(bodyText(context, p, body, color = p.inkSecondary, sizeSp = 12.5f))

            if (actionLabel != null && onAction != null) {
                addView(vSpace(context, 10f))
                val btn = actionButton(
                    context = context,
                    p = p,
                    label = actionLabel,
                    kind = ButtonKind.SECONDARY,
                    onClick = onAction,
                )
                addView(
                    btn,
                    LinearLayout.LayoutParams(
                        ViewGroup.LayoutParams.WRAP_CONTENT,
                        ViewGroup.LayoutParams.WRAP_CONTENT,
                    ),
                )
            }
        }
    }

    fun sliderRow(
        context: Context,
        p: V2Palette,
        label: String,
        minVal: Float,
        maxVal: Float,
        initial: Float,
        formatValue: (Float) -> String,
        onValue: (Float) -> Unit,
    ): LinearLayout {
        val steps = 100
        val valueText = TextView(context).apply {
            text = formatValue(initial.coerceIn(minVal, maxVal))
            setTextColor(p.ink)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 12f)
            typeface = UiKit.mono()
        }

        val trackBg = GradientDrawable().apply {
            shape = GradientDrawable.RECTANGLE
            cornerRadius = UiKit.dpF(context, 2f)
            setColor(p.well)
            setStroke(UiKit.dp(context, 1f).coerceAtLeast(1), p.border)
        }
        val trackFill = ClipDrawable(
            GradientDrawable().apply {
                shape = GradientDrawable.RECTANGLE
                cornerRadius = UiKit.dpF(context, 2f)
                setColor(p.accent)
            },
            Gravity.START,
            ClipDrawable.HORIZONTAL,
        )
        val thumbDrawable = GradientDrawable().apply {
            shape = GradientDrawable.RECTANGLE
            cornerRadius = UiKit.dpF(context, 3f)
            setColor(p.ink)
            setSize(UiKit.dp(context, 12f), UiKit.dp(context, 18f))
        }

        val seek = SeekBar(context).apply {
            max = steps
            val frac = ((initial - minVal) / (maxVal - minVal)).coerceIn(0f, 1f)
            progress = (frac * steps).toInt()
            splitTrack = false
            thumb = thumbDrawable
            progressDrawable = LayerDrawable(arrayOf(trackBg, trackFill)).apply {
                setId(0, android.R.id.background)
                setId(1, android.R.id.progress)
            }
            setPadding(UiKit.dp(context, 4f), UiKit.dp(context, 6f), UiKit.dp(context, 4f), UiKit.dp(context, 6f))
            setOnSeekBarChangeListener(
                object : SeekBar.OnSeekBarChangeListener {
                    override fun onProgressChanged(sb: SeekBar?, progress: Int, fromUser: Boolean) {
                        val value = minVal + (progress.toFloat() / steps) * (maxVal - minVal)
                        valueText.text = formatValue(value)
                        if (fromUser) onValue(value)
                    }

                    override fun onStartTrackingTouch(sb: SeekBar?) = Unit

                    override fun onStopTrackingTouch(sb: SeekBar?) = Unit
                },
            )
        }

        return LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(0, UiKit.dp(context, 6f), 0, UiKit.dp(context, 6f))

            val header = LinearLayout(context).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                addView(
                    TextView(context).apply {
                        text = label
                        setTextColor(p.inkSecondary)
                        setTextSize(TypedValue.COMPLEX_UNIT_SP, 13f)
                        layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
                    },
                )
                addView(valueText)
            }
            addView(header)
            addView(seek)
        }
    }
}
