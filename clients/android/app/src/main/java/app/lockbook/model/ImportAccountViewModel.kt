package app.lockbook.model

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.getRes
import app.lockbook.util.LbError
import app.lockbook.util.SingleMutableLiveData
import com.github.michaelbull.result.Err
import kotlinx.coroutines.launch
import timber.log.Timber

class ImportAccountViewModel(application: Application) : AndroidViewModel(application) {
    val syncModel = SyncModel()

    private val _updateImportUI = SingleMutableLiveData<UpdateImportUI>()

    val updateImportUI: LiveData<UpdateImportUI>
        get() = _updateImportUI

    init {
        viewModelScope.launch {
            val syncResult = syncModel.trySync()

            if (syncResult is Err) {
                _updateImportUI.postValue(UpdateImportUI.NotifyError(syncResult.error.toLbError(getRes())))
            }
        }
    }
}

sealed class UpdateImportUI {
    data class NotifyError(val error: LbError) : UpdateImportUI()
    object FinishedSync : UpdateImportUI()
}
