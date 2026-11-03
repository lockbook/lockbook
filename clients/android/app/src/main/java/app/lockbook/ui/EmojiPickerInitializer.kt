package app.lockbook.ui

import androidx.emoji2.text.EmojiCompat
import com.vanniktech.emoji.EmojiManager
import com.vanniktech.emoji.androidxemoji2.AndroidxEmoji2Provider
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

/** Install the catalog once per process, off the main thread, when a picker needs it. */
internal object EmojiPickerInitializer {
    private val mutex = Mutex()
    private var initialized = false

    suspend fun ensureInitialized() =
        withContext(Dispatchers.Default) {
            mutex.withLock {
                if (!initialized) {
                    val emojiCompat = EmojiCompat.get()
                    EmojiManager.install(DeviceEmojiProvider(AndroidxEmoji2Provider(emojiCompat)))
                    initialized = true
                }
            }
        }
}
