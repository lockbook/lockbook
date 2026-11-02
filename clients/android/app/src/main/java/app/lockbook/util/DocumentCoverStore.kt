package app.lockbook.util

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.ImageDecoder
import android.graphics.Matrix
import android.net.Uri
import android.os.Build
import androidx.exifinterface.media.ExifInterface
import java.io.File
import java.io.IOException
import java.security.MessageDigest

/** Device-local presentation metadata; covers do not modify document contents. Call on an IO dispatcher. */
internal class DocumentCoverStore(
    private val context: Context,
) {
    private val directory = File(context.filesDir, "document_covers")

    fun file(id: String): File {
        val key = MessageDigest.getInstance("SHA-256").digest(id.toByteArray()).joinToString("") { "%02x".format(it) }
        return File(directory, "$key.jpg")
    }

    fun save(
        id: String,
        uri: Uri,
    ) {
        val bitmap = decode(uri)
        directory.mkdirs()
        val temporary = File.createTempFile("cover", ".jpg", directory)
        try {
            temporary.outputStream().use { stream ->
                if (!bitmap.compress(Bitmap.CompressFormat.JPEG, 90, stream)) throw IOException("Unable to save cover")
            }
            if (!temporary.renameTo(file(id))) throw IOException("Unable to replace cover")
        } finally {
            bitmap.recycle()
            temporary.delete()
        }
    }

    private fun decode(uri: Uri): Bitmap {
        val resolver = context.contentResolver
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            return ImageDecoder.decodeBitmap(ImageDecoder.createSource(resolver, uri)) { decoder, info, _ ->
                val scale = 1024f / maxOf(info.size.width, info.size.height).coerceAtLeast(1024)
                decoder.setTargetSize(
                    (info.size.width * scale).toInt().coerceAtLeast(1),
                    (info.size.height * scale).toInt().coerceAtLeast(1),
                )
                decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
            }
        }
        val options = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        resolver.openInputStream(uri).use { BitmapFactory.decodeStream(it, null, options) }
        options.inSampleSize = 1
        while (maxOf(options.outWidth, options.outHeight) / options.inSampleSize > 1024) options.inSampleSize *= 2
        options.inJustDecodeBounds = false
        val bitmap =
            resolver.openInputStream(uri).use { BitmapFactory.decodeStream(it, null, options) }
                ?: throw IOException("Unreadable image")
        val orientation =
            runCatching {
                resolver.openInputStream(uri).use { stream ->
                    stream?.let { ExifInterface(it).getAttributeInt(ExifInterface.TAG_ORIENTATION, ExifInterface.ORIENTATION_NORMAL) }
                        ?: ExifInterface.ORIENTATION_NORMAL
                }
            }.getOrDefault(ExifInterface.ORIENTATION_NORMAL)
        val matrix =
            Matrix().apply {
                when (orientation) {
                    ExifInterface.ORIENTATION_FLIP_HORIZONTAL -> {
                        setScale(-1f, 1f)
                    }

                    ExifInterface.ORIENTATION_ROTATE_180 -> {
                        setRotate(180f)
                    }

                    ExifInterface.ORIENTATION_FLIP_VERTICAL -> {
                        setScale(1f, -1f)
                    }

                    ExifInterface.ORIENTATION_TRANSPOSE -> {
                        setRotate(90f)
                        postScale(-1f, 1f)
                    }

                    ExifInterface.ORIENTATION_ROTATE_90 -> {
                        setRotate(90f)
                    }

                    ExifInterface.ORIENTATION_TRANSVERSE -> {
                        setRotate(270f)
                        postScale(-1f, 1f)
                    }

                    ExifInterface.ORIENTATION_ROTATE_270 -> {
                        setRotate(270f)
                    }
                }
            }
        return Bitmap.createBitmap(bitmap, 0, 0, bitmap.width, bitmap.height, matrix, true).also {
            if (it !== bitmap) bitmap.recycle()
        }
    }
}
