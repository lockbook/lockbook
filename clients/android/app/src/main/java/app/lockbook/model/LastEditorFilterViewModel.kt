package app.lockbook.model

import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import net.lockbook.File

internal data class LastEditorFilterState(
    val usernames: List<String> = emptyList(),
    val selected: Set<String>? = null,
    val documentTypes: Set<RecentDocumentType> = emptySet(),
) {
    val isVisible: Boolean get() = usernames.size > 1

    fun includes(username: String): Boolean = !isVisible || selected == null || username in selected

    fun withContacts(
        contacts: List<String>,
        yourself: String,
    ): LastEditorFilterState = copy(usernames = listOf(yourself) + contacts.filter { it.isNotBlank() && it != yourself }.distinct())

    fun withUserEnabled(
        username: String,
        enabled: Boolean,
    ): LastEditorFilterState {
        val users = (selected ?: usernames.toSet()).toMutableSet()
        if (enabled) users.add(username) else users.remove(username)
        return copy(selected = users)
    }

    fun recentUsernames(files: List<File>): List<String> {
        val editors = files.mapTo(mutableSetOf()) { it.lastModifiedBy }
        return usernames.filter { it in editors }
    }

    fun withDocumentTypeToggled(type: RecentDocumentType): LastEditorFilterState =
        copy(documentTypes = if (type in documentTypes) documentTypes - type else documentTypes + type)

    fun filterRecents(files: List<File>): List<File> =
        files.filter { file ->
            includes(file.lastModifiedBy) && (documentTypes.isEmpty() || documentTypes.any { it.matches(file.name) })
        }

    fun filter(files: List<File>): List<File> = files.filter { includes(it.lastModifiedBy) }
}

internal class LastEditorFilterViewModel : ViewModel() {
    private val mutableState = MutableLiveData(LastEditorFilterState())
    val state: LiveData<LastEditorFilterState> = mutableState

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
