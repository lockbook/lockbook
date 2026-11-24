package app.lockbook.ui

import android.content.Context
import android.content.res.ColorStateList
import android.graphics.Canvas
import android.graphics.ColorFilter
import android.graphics.Paint
import android.graphics.PixelFormat
import android.graphics.drawable.Drawable
import android.text.TextPaint
import android.text.style.TextAppearanceSpan
import android.util.AttributeSet
import androidx.core.content.ContextCompat
import androidx.core.graphics.ColorUtils
import app.lockbook.R
import app.lockbook.util.harmonizedIdentityColors
import com.google.android.material.chip.Chip
import com.google.android.material.color.MaterialColors

/** A username and initial avatar, with a harmonized fill for checked or selected states. */
class UsernameChip
    @JvmOverloads
    constructor(
        context: Context,
        attrs: AttributeSet? = null,
        defStyleAttr: Int = com.google.android.material.R.attr.chipStyle,
    ) : Chip(context, attrs, defStyleAttr) {
        private var boundUsername: String? = null

        fun bindUsername(username: String) {
            if (username == boundUsername) return
            boundUsername = username
            text = username
            chipIcon = EditorAvatar(context, username)
            val colors = harmonizedIdentityColors(context, username)
            val surface = MaterialColors.getColor(this, com.google.android.material.R.attr.colorSurface)
            val foreground = MaterialColors.getColor(this, com.google.android.material.R.attr.colorOnSurface)
            val secondary = MaterialColors.getColor(this, com.google.android.material.R.attr.colorOnSurfaceVariant)
            // Reuse the avatar's harmonized hue as a quiet wash over the page surface.
            val selectedFill =
                ColorUtils.blendARGB(
                    surface,
                    colors.accentContainer,
                    if (MaterialColors.isColorLight(surface)) 0.5f else 0.35f,
                )
            val states =
                arrayOf(
                    intArrayOf(-android.R.attr.state_enabled),
                    intArrayOf(android.R.attr.state_checked),
                    intArrayOf(android.R.attr.state_selected),
                    intArrayOf(),
                )
            chipBackgroundColor = ColorStateList(states, intArrayOf(surface, selectedFill, selectedFill, surface))
            setTextColor(
                ColorStateList(
                    states,
                    intArrayOf(
                        ColorUtils.setAlphaComponent(foreground, 97),
                        colors.onAccentContainer,
                        colors.onAccentContainer,
                        secondary,
                    ),
                ),
            )
            rippleColor =
                ColorStateList(
                    states,
                    intArrayOf(
                        android.graphics.Color.TRANSPARENT,
                        ColorUtils.setAlphaComponent(colors.onAccentContainer, 31),
                        ColorUtils.setAlphaComponent(colors.onAccentContainer, 31),
                        ColorUtils.setAlphaComponent(secondary, 31),
                    ),
                )
        }
    }

private class EditorAvatar(
    context: Context,
    username: String,
) : Drawable() {
    private val colors = harmonizedIdentityColors(context, username)
    private val initial = username.take(1).uppercase()
    private val background =
        ContextCompat.getDrawable(context, R.drawable.username_avatar_background)!!.mutate().apply {
            setTint(colors.accentContainer)
        }
    private val paint =
        TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
            TextAppearanceSpan(context, com.google.android.material.R.style.TextAppearance_Material3_LabelMedium).updateMeasureState(this)
            textAlign = Paint.Align.CENTER
            color = colors.onAccentContainer
        }

    override fun getIntrinsicWidth(): Int = background.intrinsicWidth

    override fun getIntrinsicHeight(): Int = background.intrinsicHeight

    override fun draw(canvas: Canvas) {
        background.bounds = bounds
        background.draw(canvas)
        canvas.drawText(initial, bounds.exactCenterX(), bounds.exactCenterY() - (paint.ascent() + paint.descent()) / 2f, paint)
    }

    override fun setAlpha(alpha: Int) {
        background.alpha = alpha
        paint.alpha = alpha
        invalidateSelf()
    }

    override fun setColorFilter(colorFilter: ColorFilter?) {
        background.colorFilter = colorFilter
        paint.colorFilter = colorFilter
        invalidateSelf()
    }

    @Deprecated("Deprecated in Android")
    override fun getOpacity(): Int = PixelFormat.TRANSLUCENT
}
