package app.lockbook.util

import java.util.Locale

/** Shared filename classification for file icons and document filters. */
class ExtensionHelper(
    val fileName: String,
) {
    private val extension = fileName.substringAfterLast('.', "").lowercase(Locale.ROOT)

    val isNote: Boolean get() = extension == "md" || extension == "txt"

    val isDrawing: Boolean get() = extension == "svg"

    val isImage: Boolean get() = extension in IMAGE_EXTENSIONS

    val isPdf: Boolean get() = extension == "pdf"

    private companion object {
        // SVG is classified as a drawing rather than an image.
        private val IMAGE_EXTENSIONS =
            setOf(
                "jpg",
                "jpeg",
                "jpe",
                "jfif",
                "png",
                "apng",
                "gif",
                "webp",
                "bmp",
                "dib",
                "wbmp",
                "tif",
                "tiff",
                "heic",
                "heif",
                "hif",
                "avif",
                "avifs",
                "jxl",
                "ico",
                "cur",
                "tga",
                "targa",
                "pnm",
                "pbm",
                "pgm",
                "ppm",
                "pam",
                "pfm",
                "pcx",
                "dds",
                "exr",
                "hdr",
                "rgbe",
                "psd",
                "raw",
                "dng",
                "cr2",
                "cr3",
                "crw",
                "nef",
                "nrw",
                "arw",
                "srf",
                "sr2",
                "raf",
                "orf",
                "rw2",
                "rwl",
                "pef",
                "srw",
                "j2k",
                "jp2",
                "jpx",
                "jpf",
                "jpm",
                "mj2",
            )
    }
}
