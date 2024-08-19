package app.lockbook.model

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.R
import app.lockbook.ui.UsageBarPreference
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

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
        when (val usageResult = CoreModel.getUsage()) {
            is Ok -> when (val uncompressedUsageResult = CoreModel.getUncompressedUsage()) {
                is Ok ->
                    if (usageResult.value.dataCap.exact == UsageBarPreference.PAID_TIER_USAGE_BYTES) {
                        when (val subscriptionResult = CoreModel.getSubscriptionInfo()) {
                            is Ok -> _determineSettingsInfo.postValue(SettingsInfo(usageResult.value, uncompressedUsageResult.value, subscriptionResult.value))
                            is Err -> _notifyError.postValue(subscriptionResult.error.toLbError(getRes()))
                        }
                    } else {
                        _determineSettingsInfo.postValue(SettingsInfo(usageResult.value, uncompressedUsageResult.value, null))
                    }
                is Err -> _notifyError.postValue(uncompressedUsageResult.error.toLbError(getRes()))
            }
            is Err -> _notifyError.postValue(usageResult.error.toLbError(getRes()))
        }
    }

    fun cancelSubscription() {
        viewModelScope.launch(Dispatchers.IO) {
            when (val cancelResult = CoreModel.cancelSubscription()) {
                is Ok -> {
                    _sendBreadcrumb.postValue(getString(R.string.settings_cancel_completed))
                    computeUsage()
                }
                is Err -> _notifyError.postValue(cancelResult.error.toLbError(getRes()))
            }
        }
    }

    fun deleteAccount() {
        viewModelScope.launch(Dispatchers.IO) {
            when (val cancelResult = CoreModel.deleteAccount()) {
                is Ok -> _exit.postValue(Unit)
                is Err -> _notifyError.postValue(cancelResult.error.toLbError(getRes()))
            }
        }
    }

    fun logout() {
        viewModelScope.launch(Dispatchers.IO) {
            CoreModel.logout()
            _exit.postValue(Unit)
        }
    }
}

data class SettingsInfo(
    val usage: UsageMetrics,
    val uncompressedUsage: UsageItemMetric,
    val subscriptionInfo: SubscriptionInfo?
)
