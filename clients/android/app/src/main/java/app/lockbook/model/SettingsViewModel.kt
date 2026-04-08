package app.lockbook.model

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.R
import app.lockbook.util.*
import app.lockbook.workspace.LbStatus
import app.lockbook.workspace.SpaceUsed
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.Json
import net.lockbook.Lb
import net.lockbook.LbError
import net.lockbook.SubscriptionInfo

class SettingsViewModel(application: Application) : AndroidViewModel(application) {
    private val _sendBreadcrumb = SingleMutableLiveData<String>()
    private val _determineSettingsInfo = MutableLiveData<SettingsInfo>()
    private val _exit = SingleMutableLiveData<Unit>()
    private val _notifyError = SingleMutableLiveData<LbError>()

    val sendBreadcrumb: LiveData<String>
        get() = _sendBreadcrumb

    val determineSettingsInfo: LiveData<SettingsInfo>
        get() = _determineSettingsInfo

    val exit: LiveData<Unit>
        get() = _exit

    val notifyError: LiveData<LbError>
        get() = _notifyError

    private val jsonParser = Json {
        ignoreUnknownKeys = true
    }

    init {
        updateUsage()
    }

    fun updateUsage() {
        viewModelScope.launch(Dispatchers.IO) {
            computeUsage()
        }
    }

    private fun computeUsage() {
        try {
            val raw = Lb.getStatus()
            val status: LbStatus = jsonParser.decodeFromString(raw)
            val subscriptionInfo = Lb.getSubscriptionInfo()

            status.spaceUsed?.let {
                _determineSettingsInfo.postValue(SettingsInfo(it, subscriptionInfo))
            }
        } catch (err: LbError) {
            _notifyError.postValue(err)
        }
    }

    fun cancelSubscription() {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                Lb.cancelSubscription()
                _sendBreadcrumb.postValue(getString(R.string.settings_cancel_completed))
                computeUsage()
            } catch (err: LbError) {
                _notifyError.postValue(err)
            }
        }
    }

    fun deleteAccount() {
        try {
            Lb.deleteAccount()
            _exit.postValue(Unit)
        } catch (err: LbError) {
            _notifyError.postValue(err)
        }
    }

    fun logout() {
        viewModelScope.launch(Dispatchers.IO) {
            Lb.logout()
            _exit.postValue(Unit)
        }
    }
}

data class SettingsInfo(
    val usage: SpaceUsed,
    val subscriptionInfo: SubscriptionInfo?
)
