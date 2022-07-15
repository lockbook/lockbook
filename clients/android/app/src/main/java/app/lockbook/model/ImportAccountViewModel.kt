package app.lockbook.model

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.util.LbError
import app.lockbook.util.SingleMutableLiveData
import app.lockbook.util.getRes
import com.github.michaelbull.result.Err
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

class ImportAccountViewModel(application: Application) : AndroidViewModel(application) {
    val syncModel = SyncModel()

    var isErrorVisible = false
    private val _updateImportUI = SingleMutableLiveData<UpdateImportUI>()

    val updateImportUI: LiveData<UpdateImportUI>
        get() = _updateImportUI

    init {
        viewModelScope.launch(Dispatchers.IO) {
            val syncResult = syncModel.trySync()

            if (syncResult is Err) {
                isErrorVisible = true
                _updateImportUI.postValue(UpdateImportUI.NotifyError(syncResult.error.toLbError(getRes())))
            } else {
                _updateImportUI.postValue(UpdateImportUI.FinishedSync)
            }
        }
    }
}

sealed class UpdateImportUI {
    data class NotifyError(val error: LbError) : UpdateImportUI()
    object FinishedSync : UpdateImportUI()
}
