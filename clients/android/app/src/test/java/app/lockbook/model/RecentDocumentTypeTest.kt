package app.lockbook.model

import app.lockbook.util.ExtensionHelper
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class RecentDocumentTypeTest {
    @Test
    fun `notes and drawings use exact case insensitive extensions`() {
        assertTrue(RecentDocumentType.Notes.matches("project.notes.MD"))
        assertTrue(RecentDocumentType.Drawings.matches("sketch.SVG"))
        assertFalse(RecentDocumentType.Images.matches("sketch.svg"))
        assertFalse(RecentDocumentType.Notes.matches("notes.md.backup"))
        assertFalse(RecentDocumentType.Notes.matches("md"))
    }

    @Test
    fun `images include common legacy modern and camera formats`() {
        val extensions =
            listOf(
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
                "avif",
                "jxl",
                "ico",
                "cur",
                "tga",
                "ppm",
                "psd",
                "dng",
                "cr2",
                "cr3",
                "nef",
                "arw",
                "raf",
                "orf",
                "rw2",
                "raw",
                "jp2",
                "exr",
                "hdr",
            )
        for (extension in extensions) {
            val name = "photo.${extension.uppercase()}"
            assertTrue(name, RecentDocumentType.Images.matches(name))
            assertTrue(name, ExtensionHelper(name).isImage)
        }
        assertFalse(RecentDocumentType.Images.matches("report.pdf"))
        assertFalse(RecentDocumentType.Images.matches("image.png.txt"))
        assertFalse(RecentDocumentType.Images.matches("no-extension"))
    }
}
