package app.lockbook.model

import app.lockbook.util.fileExtension
import app.lockbook.util.isImageFileName

internal enum class RecentDocumentType {
    Notes,
    Drawings,
    Images,
    ;

    fun matches(name: String): Boolean =
        when (this) {
            Notes -> fileExtension(name) == "md"
            Drawings -> fileExtension(name) == "svg"
            Images -> isImageFileName(name)
        }
}
