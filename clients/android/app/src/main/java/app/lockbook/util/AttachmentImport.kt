package app.lockbook.util

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import java.io.ByteArrayOutputStream

/** Product-wide maximum size for files imported from outside Lockbook. */
const val MAX_ATTACHMENT_SIZE_BYTES = 25 * 1024 * 1024

class AttachmentTooLargeException : Exception()

fun readAttachmentBytes(
    context: Context,
    uri: Uri,
    maxBytes: Int = MAX_ATTACHMENT_SIZE_BYTES,
): ByteArray? {
    val resolver = context.contentResolver
    val expectedSize =
        runCatching {
            resolver.openAssetFileDescriptor(uri, "r")?.use { descriptor ->
                descriptor.length.takeIf { it >= 0 }
            }
        }.getOrNull()
            ?: runCatching {
                resolver.query(uri, arrayOf(OpenableColumns.SIZE), null, null, null)?.use { cursor ->
                    val index = cursor.getColumnIndex(OpenableColumns.SIZE)
                    if (index != -1 && cursor.moveToFirst()) cursor.getLong(index) else null
                }
            }.getOrNull()

    if (expectedSize != null && expectedSize > maxBytes) throw AttachmentTooLargeException()

    resolver.openInputStream(uri)?.use { input ->
        val initialSize = expectedSize?.takeIf { it in 1..maxBytes }?.toInt() ?: DEFAULT_BUFFER_SIZE
        val output = ByteArrayOutputStream(initialSize)
        val buffer = ByteArray(DEFAULT_BUFFER_SIZE)
        var total = 0
        while (true) {
            val count = input.read(buffer)
            if (count < 0) break
            total += count
            if (total > maxBytes) throw AttachmentTooLargeException()
            output.write(buffer, 0, count)
        }
        return output.toByteArray()
    }

    return null
}
