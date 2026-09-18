package app.lockbook.util

import android.content.ContentResolver
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
    private val imageExtensions =
        setOf("avif", "bmp", "gif", "heic", "heif", "jpeg", "jpg", "png", "tif", "tiff", "webp")

    /** Must be called from a background dispatcher. */
    fun stage(
        context: Context,
        uri: Uri,
        displayNameHint: String? = null,
        isImageHint: Boolean? = null,
    ): StagedAttachment? {
        val resolver = context.contentResolver
        val displayName = displayNameHint ?: displayName(uri, resolver)
        val safeName =
            displayName
                .substringAfterLast('/')
                .replace('\u0000', '_')
                .take(230)
                .ifBlank { "attachment" }
        val isImage = isImageHint ?: isImage(uri, displayName, resolver)
        val file = stageFile(context, uri, resolver) ?: return null
        return StagedAttachment(file.absolutePath, safeName, isImage)
    }

    private fun stageFile(
        context: Context,
        uri: Uri,
        resolver: ContentResolver,
    ): File? {
        if (uri.scheme == ContentResolver.SCHEME_FILE) {
            val file = File(uri.path.orEmpty())
            return file.takeIf { it.isFile && it.length() <= MAX_CONTENT_SIZE }
        }

        val attachmentDir = File(context.cacheDir, "attachments").apply { mkdirs() }
        val stagedFile = File.createTempFile("attachment_", ".tmp", attachmentDir)
        val input = resolver.openInputStream(uri)
        if (input == null) {
            stagedFile.delete()
            return null
        }

        var total = 0L
        var completed = false
        try {
            input.use { source ->
                stagedFile.outputStream().use { destination ->
                    val buffer = ByteArray(DEFAULT_BUFFER_SIZE)
                    while (true) {
                        val read = source.read(buffer)
                        if (read <= 0) break
                        total += read
                        if (total > MAX_CONTENT_SIZE) return null
                        destination.write(buffer, 0, read)
                    }
                }
            }
            completed = true
            return stagedFile
        } finally {
            if (!completed) stagedFile.delete()
        }
    }

    private fun displayName(
        uri: Uri,
        resolver: ContentResolver,
    ): String {
        try {
            resolver
                .query(
                    uri,
                    arrayOf(OpenableColumns.DISPLAY_NAME),
                    null,
                    null,
                    null,
                )?.use { cursor ->
                    val index = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                    if (index != -1 && cursor.moveToFirst()) {
                        cursor.getString(index)?.let { return it }
                    }
                }
        } catch (_: Exception) {
            // Some document providers do not implement metadata queries.
        }
        return uri.lastPathSegment ?: "attachment"
    }

    private fun isImage(
        uri: Uri,
        displayName: String,
        resolver: ContentResolver,
    ): Boolean {
        val mime =
            try {
                resolver.getType(uri)
            } catch (_: Exception) {
                null
            }
        if (mime?.startsWith("image/") == true) return true
        return displayName.substringAfterLast('.', "").lowercase() in imageExtensions
    }
}
