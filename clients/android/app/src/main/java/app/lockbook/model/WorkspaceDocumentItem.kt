package app.lockbook.model

import net.lockbook.File

internal data class WorkspaceDocumentItem(
    val file: File,
    val name: String = file.name,
    val modified: Long = file.lastModified,
    val cursor: Int = 0,
) {
    fun sameContentAs(other: WorkspaceDocumentItem): Boolean =
        file.id == other.file.id && name == other.name && modified == other.modified && cursor == other.cursor
}

internal fun workspaceDocumentItems(
    pins: List<PinnedFile>,
    documents: List<File>,
): List<WorkspaceDocumentItem> {
    val editable =
        documents.filter {
            it.type == File.FileType.Document &&
                it.name.endsWith(".md", ignoreCase = true)
        }
    val byId = editable.associateBy { it.id }
    val pinned = pins.mapNotNull { byId[it.id] }.distinctBy { it.id }
    val pinnedIds = pinned.map { it.id }.toSet()
    val recent = recentDocuments(editable).filter { it.id !in pinnedIds }.take(12)
    return (pinned + recent).map(::WorkspaceDocumentItem)
}
