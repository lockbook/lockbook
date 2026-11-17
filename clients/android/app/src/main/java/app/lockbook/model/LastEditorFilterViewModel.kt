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

    fun filter(files: List<File>): List<File> = files.filter { includes(it.lastModifiedBy) }
}

internal class LastEditorFilterViewModel : ViewModel() {
    private val mutableState = MutableLiveData(LastEditorFilterState())
    val state: LiveData<LastEditorFilterState> = mutableState

    fun updateContacts(
        usernames: List<String>,
        yourself: String,
    ) {
        val users = listOf(yourself) + usernames.filter { it.isNotBlank() && it != yourself }.distinct().sorted()
        mutableState.value = mutableState.value!!.copy(usernames = users)
    }

    fun selectAll() {
        mutableState.value = mutableState.value!!.copy(selected = null)
    }

    fun deselectAll() {
        mutableState.value = mutableState.value!!.copy(selected = emptySet())
    }

    fun toggle(username: String) {
        val current = mutableState.value!!
        val selected = (current.selected ?: current.usernames.toSet()).toMutableSet()
        if (!selected.remove(username)) selected.add(username)
        mutableState.value = current.copy(selected = selected)
    }
}
