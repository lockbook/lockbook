package app.lockbook.ui

import android.content.Context
import android.content.res.ColorStateList
import android.graphics.Canvas
import android.graphics.ColorFilter
import android.graphics.Paint
import android.graphics.PixelFormat
import android.graphics.Typeface
import android.graphics.drawable.Drawable
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

        init {
            isCheckedIconVisible = false
            isCloseIconVisible = false
            isChipIconVisible = true
            chipIconSize = 24f * resources.displayMetrics.density
            chipIconTint = null
            chipStrokeColor = ContextCompat.getColorStateList(this.context, R.color.recent_editor_chip_stroke)
        }

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
    private var drawableAlpha = 255
    private val paint =
        Paint(Paint.ANTI_ALIAS_FLAG).apply {
            textAlign = Paint.Align.CENTER
            typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
        }

    override fun draw(canvas: Canvas) {
        val cx = bounds.exactCenterX()
        val cy = bounds.exactCenterY()
        paint.color = colors.accentContainer
        paint.alpha = drawableAlpha
        canvas.drawCircle(cx, cy, minOf(bounds.width(), bounds.height()) / 2f, paint)
        paint.color = colors.onAccentContainer
        paint.alpha = drawableAlpha
        paint.textSize = bounds.height() * 0.55f
        canvas.drawText(initial, cx, cy - (paint.ascent() + paint.descent()) / 2f, paint)
    }

    override fun setAlpha(alpha: Int) {
        drawableAlpha = alpha
        invalidateSelf()
    }

    override fun setColorFilter(colorFilter: ColorFilter?) {
        paint.colorFilter = colorFilter
        invalidateSelf()
    }

    @Deprecated("Deprecated in Android")
    override fun getOpacity(): Int = PixelFormat.TRANSLUCENT
}
