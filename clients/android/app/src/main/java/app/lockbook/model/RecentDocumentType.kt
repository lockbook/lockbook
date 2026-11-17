package app.lockbook.model

import app.lockbook.util.ExtensionHelper

internal enum class RecentDocumentType {
    Notes,
    Drawings,
    Images,
    ;

    fun matches(name: String): Boolean {
        val extension = ExtensionHelper(name)
        return when (this) {
            Notes -> extension.isNote
            Drawings -> extension.isDrawing
            Images -> extension.isImage
        }
    }
}
