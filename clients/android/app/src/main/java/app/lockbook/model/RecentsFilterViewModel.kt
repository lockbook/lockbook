package app.lockbook.model

import androidx.lifecycle.LiveData
import androidx.lifecycle.MediatorLiveData
import androidx.lifecycle.Observer
import androidx.lifecycle.ViewModel
import app.lockbook.util.ExtensionHelper
import net.lockbook.File
import net.lockbook.SharingContact

internal data class EditorFilter(
    val usernames: List<String> = emptyList(),
    val selected: Set<String>? = null,
) {
    val hasMultipleEditors: Boolean get() = usernames.size > 1

    fun withContacts(
        contacts: List<String>,
        yourself: String,
    ): EditorFilter = copy(usernames = listOf(yourself) + contacts.filter { it.isNotBlank() && it != yourself }.distinct())

    fun withUserEnabled(
        username: String,
        enabled: Boolean,
    ): EditorFilter {
        val users = (selected ?: usernames.toSet()).toMutableSet()
        if (enabled) users.add(username) else users.remove(username)
        return copy(selected = users)
    }

    // Use the full, unfiltered Recents list so deselected editor chips stay available.
    fun recentUsernames(files: List<File>): List<String> {
        val editors = files.mapTo(mutableSetOf()) { it.lastModifiedBy }
        return usernames.filter { it in editors }
    }

    fun filter(files: List<File>): List<File> = files.filter { includes(it.lastModifiedBy) }

    fun includes(username: String): Boolean = !hasMultipleEditors || selected == null || username in selected
}

internal data class RecentsFilterState(
    val editorFilter: EditorFilter = EditorFilter(),
    val documentTypes: Set<RecentDocumentType> = emptySet(),
) {
    // An empty type selection includes all document types.
    fun withDocumentTypeToggled(type: RecentDocumentType): RecentsFilterState =
        copy(documentTypes = if (type in documentTypes) documentTypes - type else documentTypes + type)

    fun filterRecents(files: List<File>): List<File> =
        files.filter { file ->
            editorFilter.includes(file.lastModifiedBy) && (documentTypes.isEmpty() || documentTypes.any { it.matches(file.name) })
        }
}

internal class RecentsFilterViewModel : ViewModel() {
    private val mutableState = MediatorLiveData(RecentsFilterState())
    val state: LiveData<RecentsFilterState> = mutableState

    private val contactsObserver =
        Observer<List<SharingContact>> { contacts ->
            val username = AccountStore.username ?: return@Observer
            val current = mutableState.value!!
            val updated = current.copy(editorFilter = current.editorFilter.withContacts(contacts.map { it.username }, username))
            if (updated != current) mutableState.value = updated
        }

    fun observeContacts(contacts: LiveData<List<SharingContact>>) {
        mutableState.addSource(contacts, contactsObserver)
    }

    fun toggle(username: String) {
        val current = mutableState.value!!
        mutableState.value =
            current.copy(
                editorFilter = current.editorFilter.withUserEnabled(username, current.editorFilter.selected?.contains(username) == false),
            )
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
