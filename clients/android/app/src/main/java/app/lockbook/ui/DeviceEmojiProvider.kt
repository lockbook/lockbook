package app.lockbook.ui

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.ColorFilter
import android.graphics.Paint
import android.graphics.PixelFormat
import android.graphics.drawable.Drawable
import android.text.TextPaint
import com.google.android.material.color.MaterialColors
import com.vanniktech.emoji.Emoji
import com.vanniktech.emoji.EmojiAndroidProvider
import com.vanniktech.emoji.EmojiCategory

/** Keep browsing, search, variants, and saved recents within the device's native emoji coverage. */
class DeviceEmojiProvider(
    private val source: EmojiAndroidProvider,
) : EmojiAndroidProvider by source {
    private val paint = Paint().apply { textSize = 32f }

    override val categories: Array<EmojiCategory> =
        source.categories
            .map { category ->
                DeviceCategory(
                    category,
                    category.emojis.filter { paint.hasGlyph(it.unicode) }.map { emoji ->
                        object : Emoji by emoji {
                            override val variants = emoji.variants.filter { paint.hasGlyph(it.unicode) }
                        }
                    },
                )
            }.toTypedArray()

    override fun getIcon(emojiCategory: EmojiCategory): Int = source.getIcon((emojiCategory as DeviceCategory).source)

    override fun getDrawable(
        emoji: Emoji,
        context: Context,
    ): Drawable =
        DeviceEmojiDrawable(
            emoji.unicode,
            MaterialColors.getColor(context, com.google.android.material.R.attr.colorOnSurface, Color.BLACK),
        )

    private class DeviceCategory(
        val source: EmojiCategory,
        override val emojis: List<Emoji>,
    ) : EmojiCategory by source
}

/** Native color emoji keep their colors; monochrome symbols use a readable theme color. */
private class DeviceEmojiDrawable(
    private val unicode: String,
    foregroundColor: Int,
) : Drawable() {
    private val paint =
        TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
            color = foregroundColor
            textAlign = Paint.Align.CENTER
        }

    override fun draw(canvas: Canvas) {
        paint.textSize = minOf(bounds.width(), bounds.height()) * 0.8f
        val metrics = paint.fontMetrics
        val baseline = bounds.exactCenterY() - (metrics.ascent + metrics.descent) / 2
        canvas.drawText(unicode, bounds.exactCenterX(), baseline, paint)
    }

    override fun setAlpha(alpha: Int) {
        paint.alpha = alpha
        invalidateSelf()
    }

    override fun setColorFilter(colorFilter: ColorFilter?) {
        paint.colorFilter = colorFilter
        invalidateSelf()
    }

    @Suppress("OVERRIDE_DEPRECATION")
    override fun getOpacity(): Int = PixelFormat.TRANSLUCENT
}
