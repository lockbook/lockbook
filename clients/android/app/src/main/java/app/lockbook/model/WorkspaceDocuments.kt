package app.lockbook.model

import androidx.annotation.StringRes
import app.lockbook.R
import net.lockbook.File

enum class StarterDocument(
    @param:StringRes val title: Int,
    @param:StringRes val fileName: Int,
    @param:StringRes val content: Int,
) {
    Todo(R.string.starter_todo, R.string.starter_todo_file, R.string.starter_todo_content),
    Gym(R.string.starter_gym, R.string.starter_gym_file, R.string.starter_gym_content),
    Journal(R.string.starter_journal, R.string.starter_journal_file, R.string.starter_journal_content),
    Project(R.string.starter_project, R.string.starter_project_file, R.string.starter_project_content),
}

internal sealed interface WorkspaceDocumentItem {
    data class Document(
        val file: File,
        val name: String = file.name,
        val modified: Long = file.lastModified,
        val cursor: Int = 0,
    ) : WorkspaceDocumentItem

    data class Starter(
        val template: StarterDocument,
    ) : WorkspaceDocumentItem
}

internal fun workspaceDocumentItems(
    pins: List<PinnedFile>,
    documents: List<File>,
    cursors: Map<String, Int> = emptyMap(),
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
    return buildList {
        if (pinned.isNotEmpty()) {
            addAll(pinned.map { WorkspaceDocumentItem.Document(it, cursor = cursors[it.id] ?: 0) })
        }
        if (recent.isNotEmpty()) {
            addAll(recent.map { WorkspaceDocumentItem.Document(it, cursor = cursors[it.id] ?: 0) })
        }
        if (isEmpty()) {
            addAll(StarterDocument.entries.map(WorkspaceDocumentItem::Starter))
        }
    }
}
