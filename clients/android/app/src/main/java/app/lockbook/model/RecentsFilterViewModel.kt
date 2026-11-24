package app.lockbook.model

import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import app.lockbook.util.ExtensionHelper
import net.lockbook.File

internal data class RecentsFilterState(
    val usernames: List<String> = emptyList(),
    val selected: Set<String>? = null,
    val documentTypes: Set<RecentDocumentType> = emptySet(),
) {
    val hasMultipleEditors: Boolean get() = usernames.size > 1

    fun withContacts(
        contacts: List<String>,
        yourself: String,
    ): RecentsFilterState = copy(usernames = listOf(yourself) + contacts.filter { it.isNotBlank() && it != yourself }.distinct())

    fun withUserEnabled(
        username: String,
        enabled: Boolean,
    ): RecentsFilterState {
        val users = (selected ?: usernames.toSet()).toMutableSet()
        if (enabled) users.add(username) else users.remove(username)
        return copy(selected = users)
    }

    // An empty type selection includes all document types.
    fun withDocumentTypeToggled(type: RecentDocumentType): RecentsFilterState =
        copy(documentTypes = if (type in documentTypes) documentTypes - type else documentTypes + type)

    // Use the full, unfiltered Recents list so deselected editor chips stay available.
    fun recentUsernames(files: List<File>): List<String> {
        val editors = files.mapTo(mutableSetOf()) { it.lastModifiedBy }
        return usernames.filter { it in editors }
    }

    fun filterRecents(files: List<File>): List<File> =
        files.filter { file ->
            includesEditor(file.lastModifiedBy) && (documentTypes.isEmpty() || documentTypes.any { it.matches(file.name) })
        }

    // Welcome shares the editor selection, but ignores document types.
    fun filterByEditor(files: List<File>): List<File> = files.filter { includesEditor(it.lastModifiedBy) }

    fun includesEditor(username: String): Boolean = !hasMultipleEditors || selected == null || username in selected
}

internal class RecentsFilterViewModel : ViewModel() {
    private val mutableState = MutableLiveData(RecentsFilterState())
    val state: LiveData<RecentsFilterState> = mutableState

    fun updateContacts(
        usernames: List<String>,
        yourself: String,
    ) {
        val updated = mutableState.value!!.withContacts(usernames, yourself)
        if (updated != mutableState.value) mutableState.value = updated
    }

    fun toggle(username: String) {
        val current = mutableState.value!!
        mutableState.value = current.withUserEnabled(username, current.selected?.contains(username) == false)
    }

    fun toggleDocumentType(type: RecentDocumentType) {
        mutableState.value = mutableState.value!!.withDocumentTypeToggled(type)
    }
}

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
