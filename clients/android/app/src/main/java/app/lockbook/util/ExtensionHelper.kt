package app.lockbook.util

import app.lockbook.workspace.Workspace
import java.util.Locale

/** Shared filename classification for file icons and document filters. */
class ExtensionHelper(
    val fileName: String,
) {
    private val extension = fileName.substringAfterLast('.', "").lowercase(Locale.ROOT)

    val isNote: Boolean get() = extension == "md" || extension == "txt"

    val isDrawing: Boolean get() = extension == "svg"

    val isImage: Boolean get() = extension in Workspace.supportedImageExtensions

    val isPdf: Boolean get() = extension == "pdf"
}
