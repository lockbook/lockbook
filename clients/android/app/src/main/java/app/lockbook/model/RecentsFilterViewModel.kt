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
    // Editor controls are only useful when there is another known account.
    val isVisible: Boolean get() = usernames.size > 1

    // Shared editor predicate; null selection includes everyone.
    fun includesEditor(username: String): Boolean = !isVisible || selected == null || username in selected

    // Refresh the contact roster without changing selections; keep the account first and contact ranking intact.
    fun withContacts(
        contacts: List<String>,
        yourself: String,
    ): RecentsFilterState = copy(usernames = listOf(yourself) + contacts.filter { it.isNotBlank() && it != yourself }.distinct())

    // Return the state after changing one editor chip; other editor and type selections are preserved.
    fun withUserEnabled(
        username: String,
        enabled: Boolean,
    ): RecentsFilterState {
        val users = (selected ?: usernames.toSet()).toMutableSet()
        if (enabled) users.add(username) else users.remove(username)
        return copy(selected = users)
    }

    // Build editor chips from the full, unfiltered Recents list so deselected editors stay available.
    fun recentUsernames(files: List<File>): List<String> {
        val editors = files.mapTo(mutableSetOf()) { it.lastModifiedBy }
        return usernames.filter { it in editors }
    }

    // Toggle one type chip; an empty type selection leaves all document types visible.
    fun withDocumentTypeToggled(type: RecentDocumentType): RecentsFilterState =
        copy(documentTypes = if (type in documentTypes) documentTypes - type else documentTypes + type)

    // Recents applies editor AND type filters; selected types match with OR.
    fun filterRecents(files: List<File>): List<File> =
        files.filter { file ->
            includesEditor(file.lastModifiedBy) && (documentTypes.isEmpty() || documentTypes.any { it.matches(file.name) })
        }

    // Welcome shares editor selection only, before choosing pinned and recent cards.
    fun filterByEditor(files: List<File>): List<File> = files.filter { includesEditor(it.lastModifiedBy) }
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
