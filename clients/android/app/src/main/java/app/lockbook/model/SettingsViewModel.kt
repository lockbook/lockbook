package app.lockbook.model

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.getRes
import app.lockbook.ui.UsageBarPreference
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

class SettingsViewModel(application: Application) : AndroidViewModel(application) {
    private val _canceledSubscription = MutableLiveData<Unit>()
    private val _determineSettingsInfo = MutableLiveData<SettingsInfo>()
    private val _notifyError = SingleMutableLiveData<LbError>()

    val canceledSubscription: LiveData<Unit>
        get() = _canceledSubscription

    val determineSettingsInfo: LiveData<SettingsInfo>
        get() = _determineSettingsInfo

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
            when(val cancelResult = CoreModel.cancelSubscription()) {
                is Ok -> {
                    _canceledSubscription.postValue(Unit)
                    computeUsage()
                }
                is Err -> _notifyError.postValue(cancelResult.error.toLbError(getRes()))
            }
        }
    }
}

data class SettingsInfo(
    val usage: UsageMetrics,
    val uncompressedUsage: UsageItemMetric,
    val subscriptionInfo: SubscriptionInfo?
)
