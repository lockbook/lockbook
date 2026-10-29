package app.lockbook.ui

import android.graphics.Paint
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

    private class DeviceCategory(
        val source: EmojiCategory,
        override val emojis: List<Emoji>,
    ) : EmojiCategory by source
}
