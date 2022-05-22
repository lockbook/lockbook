package app.lockbook.model

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.getRes
import app.lockbook.util.LbError
import app.lockbook.util.SingleMutableLiveData
import app.lockbook.util.UsageMetrics
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

class SettingsViewModel(application: Application) : AndroidViewModel(application) {
    private val _usageDetermined = SingleMutableLiveData<UsageMetrics>()
    private val _notifyError = SingleMutableLiveData<LbError>()

    val usageDetermined: LiveData<UsageMetrics>
        get() = _usageDetermined

    val notifyError: LiveData<LbError>
        get() = _notifyError


    init {
        updateUsage()
    }

    private fun updateUsage() {
        viewModelScope.launch(Dispatchers.IO) {
            when(val usageResult = CoreModel.getUsage()) {
                is Ok -> _usageDetermined.postValue(usageResult.value)
                is Err -> _notifyError.postValue(usageResult.error.toLbError(getRes()))
            }
        }
    }

}

