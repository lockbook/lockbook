package app.lockbook.util

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.util.LruCache
import androidx.core.graphics.scale
import app.lockbook.model.MarkdownExcerpt
import app.lockbook.model.markdownExcerpt
import app.lockbook.model.savedMarkdownCursors
import app.lockbook.workspace.Workspace
import com.google.android.material.color.DynamicColorsOptions
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.sync.withPermit
import kotlinx.coroutines.withContext
import net.lockbook.Lb
import java.io.File

internal sealed interface DocumentPreview {
    data class Markdown(
        val excerpt: MarkdownExcerpt,
    ) : DocumentPreview

    data class Image(
        val bitmap: Bitmap,
        val seedColor: Int?,
    ) : DocumentPreview
}

/** Previews stay in memory; decrypted document text is not written to disk. */
internal class DocumentPreviewLoader(
    context: Context,
    private val covers: DocumentCoverStore,
) {
    private val persistence = File(context.filesDir, "ws_persistence.json")
    private var lastCursors = emptyMap<String, Int>()
    private val workers = Semaphore(2)
    private val cache =
        object : LruCache<String, DocumentPreview>(8 * 1024 * 1024) {
            override fun sizeOf(
                key: String,
                value: DocumentPreview,
            ): Int =
                when (value) {
                    is DocumentPreview.Image -> value.bitmap.byteCount
                    is DocumentPreview.Markdown -> (value.excerpt.markdown.length + value.excerpt.focusLine.length) * 2 + 64
                }
        }

    suspend fun cursorPositions(): Map<String, Int> =
        withContext(Dispatchers.IO) {
            if (!persistence.exists()) return@withContext emptyMap()
            // The editor writes asynchronously. Retry a partial write, retaining the last good snapshot on failure.
            repeat(3) {
                runCatching { savedMarkdownCursors(persistence.readText()) }.getOrNull()?.let {
                    lastCursors = it
                    return@withContext it
                }
                delay(25)
            }
            lastCursors
        }

    fun invalidate(id: String) {
        cache
            .snapshot()
            .keys
            .filter { it.startsWith("$id:") }
            .forEach(cache::remove)
    }

    private fun coverPreview(bitmap: Bitmap): DocumentPreview.Image {
        // Material's image palette extraction runs on a small sample, alongside background decoding.
        val sample = bitmap.scale(64, 64)
        val seed =
            try {
                DynamicColorsOptions
                    .Builder()
                    .setContentBasedSource(sample)
                    .build()
                    .contentBasedSeedColor
            } finally {
                if (sample !== bitmap) sample.recycle()
            }
        return DocumentPreview.Image(bitmap, seed)
    }

    suspend fun load(
        id: String,
        name: String,
        modified: Long,
        cursor: Int,
    ): DocumentPreview? =
        workers.withPermit {
            withContext(Dispatchers.IO) {
                val custom = covers.file(id)
                val key = "$id:$modified:$cursor:${custom.lastModified()}:${custom.length()}"
                cache.get(key)?.let { return@withContext it }
                val preview =
                    if (custom.exists()) {
                        BitmapFactory
                            .decodeFile(custom.path, BitmapFactory.Options().apply { inSampleSize = 2 })
                            ?.let(::coverPreview)
                    } else if (name.endsWith(".md", ignoreCase = true)) {
                        val content = Lb.readDocument(id)
                        val offset = Workspace.markdownCursorUtf16Native(content, cursor)
                        DocumentPreview.Markdown(markdownExcerpt(content, offset))
                    } else {
                        null
                    }
                preview?.also { cache.put(key, it) }
            }
        }
}
