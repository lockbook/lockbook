package app.lockbook.model

import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import net.lockbook.File

internal data class LastEditorFilterState(
    val usernames: List<String> = emptyList(),
    val selected: Set<String>? = null,
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

    fun selectAll() {
        mutableState.value = mutableState.value!!.copy(selected = null)
    }

    fun deselectAll() {
        mutableState.value = mutableState.value!!.copy(selected = emptySet())
    }

    fun toggle(username: String) {
        val current = mutableState.value!!
        setEnabled(username, current.selected?.contains(username) == false)
    }

    fun setEnabled(
        username: String,
        enabled: Boolean,
    ) {
        mutableState.value = mutableState.value!!.withUserEnabled(username, enabled)
    }
}
