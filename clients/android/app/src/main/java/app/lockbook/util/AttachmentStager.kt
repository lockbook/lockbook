package app.lockbook.util

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import java.io.File

internal data class StagedAttachment(
    val tempPath: String,
    val name: String,
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
                        if (total > MAX_ATTACHMENT_SIZE_BYTES) return null
                        destination.write(buffer, 0, count)
                    }
                }
            }
            if (file.length() == 0L) return null
            complete = true
            return StagedAttachment(file.absolutePath, name)
        } finally {
            if (!complete || file.length() == 0L || file.length() > MAX_ATTACHMENT_SIZE_BYTES) file.delete()
        }
    }
}
