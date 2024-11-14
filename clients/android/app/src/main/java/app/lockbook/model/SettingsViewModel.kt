package app.lockbook.model

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.R
import app.lockbook.util.*
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import net.lockbook.Lb
import net.lockbook.LbError
import net.lockbook.SubscriptionInfo
import net.lockbook.Usage

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
            val usage = Lb.getUsage()
            val uncompressedUsage = Lb.getUncompressedUsage()
            val subscriptionInfo = Lb.getSubscriptionInfo()

            _determineSettingsInfo.postValue(SettingsInfo(usage, uncompressedUsage, subscriptionInfo))
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
    val usage: Usage,
    val uncompressedUsage: Usage.UsageItemMetric,
    val subscriptionInfo: SubscriptionInfo?
)
