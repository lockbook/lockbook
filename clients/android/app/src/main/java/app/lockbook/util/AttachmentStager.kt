package app.lockbook.util

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import java.io.File

internal data class StagedAttachment(
    val path: String,
    val name: String,
    val isImage: Boolean,
)

internal object AttachmentStager {
    /** Copy while the picker grant is valid; the queue owns the resulting temporary file. */
    fun stage(
        context: Context,
        uri: Uri,
        nameHint: String? = null,
    ): StagedAttachment? {
        val resolver = context.contentResolver
        val name =
            (
                nameHint ?: runCatching {
                    resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
                        if (cursor.moveToFirst()) cursor.getString(0) else null
                    }
                }.getOrNull() ?: uri.lastPathSegment ?: "attachment"
            ).substringAfterLast('/')
                .substringAfterLast('\\')
                .replace('\u0000', '_')
                .take(230)
                .ifBlank { "attachment" }
        val mime = runCatching { resolver.getType(uri) }.getOrNull()
        val isImage =
            mime?.startsWith("image/") == true ||
                name.substringAfterLast('.', "").lowercase() in
                setOf("avif", "bmp", "gif", "heic", "heif", "jpeg", "jpg", "png", "tif", "tiff", "webp")
        val dir = File(context.cacheDir, "attachments").apply { mkdirs() }
        val file = File.createTempFile("attachment_", ".tmp", dir)
        var complete = false
        try {
            val input = resolver.openInputStream(uri) ?: return null
            input.use { source ->
                file.outputStream().use { destination ->
                    val buffer = ByteArray(DEFAULT_BUFFER_SIZE)
                    var total = 0L
                    while (true) {
                        val count = source.read(buffer)
                        if (count < 0) break
                        total += count
                        if (total > MAX_CONTENT_SIZE) return null
                        destination.write(buffer, 0, count)
                    }
                }
            }
            if (file.length() == 0L) return null
            complete = true
            return StagedAttachment(file.absolutePath, name, isImage)
        } finally {
            if (!complete || file.length() == 0L || file.length() > MAX_CONTENT_SIZE) file.delete()
        }
    }
}
