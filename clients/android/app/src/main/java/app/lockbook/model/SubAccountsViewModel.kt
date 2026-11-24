package app.lockbook.model

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.ui.UsageBarPreference
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.lockbook.Account
import net.lockbook.Lb
import net.lockbook.LbError
import net.lockbook.SubAccount
import timber.log.Timber

const val SUB_ACCOUNT_SEPARATOR = '+'

fun Account.isSubAccount(): Boolean = username.contains(SUB_ACCOUNT_SEPARATOR)

fun subAccountSuggestions(
    query: String,
    subAccounts: List<String>,
    parent: String,
    shared: Collection<String>,
): List<String> {
    val prefix = query.trim().lowercase()
    val labelStart = parent.lowercase() + SUB_ACCOUNT_SEPARATOR
    val sharedWith = shared.map { it.lowercase() }

    return subAccounts
        .filter { name ->
            val lower = name.lowercase()
            val label = lower.removePrefix(labelStart)
            when {
                lower == prefix -> false
                prefix.isEmpty() -> lower !in sharedWith
                else -> lower.startsWith(prefix) || label.startsWith(prefix) || (SUB_ACCOUNT_SEPARATOR + label).startsWith(prefix)
            }
        }.sortedWith(compareBy({ it.length }, { it.lowercase() }))
        .take(5)
}

data class SubAccountsState(
    val subAccounts: List<SubAccount> = emptyList(),
    val loaded: Boolean = false,
    val loadError: LbError? = null,
    val isPremium: Boolean? = null,
    val working: Boolean = false,
)

class SubAccountsViewModel(
    application: Application,
) : AndroidViewModel(application) {
    val parent: String = runCatching { Lb.getAccount().username }.getOrDefault("")

    private val _state = MutableLiveData(SubAccountsState())
    val state: LiveData<SubAccountsState>
        get() = _state

    fun load() {
        viewModelScope.launch {
            try {
                val subAccounts = withContext(Dispatchers.IO) { Lb.listSubAccounts().toList() }
                update { copy(subAccounts = subAccounts, loaded = true, loadError = null) }
            } catch (err: LbError) {
                update { copy(loadError = err) }
            }
        }

        if (_state.value?.isPremium == null) {
            viewModelScope.launch {
                try {
                    val usage = withContext(Dispatchers.IO) { Lb.refreshUsage() }
                    update { copy(isPremium = usage.dataCap?.exact == UsageBarPreference.PAID_TIER_USAGE_BYTES) }
                } catch (err: LbError) {
                    Timber.e("Unable to load usage: %s", err.msg)
                }
            }
        }
    }

    suspend fun create(label: String): String = run { Lb.createSubAccount(label) }

    suspend fun delete(username: String) {
        run { Lb.deleteSubAccount(username) }
        update { copy(subAccounts = subAccounts.filter { it.username != username }) }
    }

    suspend fun exportKey(username: String): String = run(reload = false) { Lb.exportSubAccountPrivateKey(username) }

    suspend fun exportPhrase(username: String): String = run(reload = false) { Lb.exportSubAccountPhrase(username) }

    private suspend fun <T> run(
        reload: Boolean = true,
        work: () -> T,
    ): T {
        update { copy(working = true) }
        try {
            val result = withContext(Dispatchers.IO) { work() }
            if (reload) load()
            return result
        } finally {
            update { copy(working = false) }
        }
    }

    private fun update(change: SubAccountsState.() -> SubAccountsState) {
        _state.value = (_state.value ?: SubAccountsState()).change()
    }
}
