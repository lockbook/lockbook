package app.lockbook.model

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.util.SingleMutableLiveData
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import net.lockbook.LbError

class ImportAccountViewModel(application: Application) : AndroidViewModel(application) {
    val syncModel = SyncModel()

    var isErrorVisible = false
    private val _updateImportUI = SingleMutableLiveData<UpdateImportUI>()

    val updateImportUI: LiveData<UpdateImportUI>
        get() = _updateImportUI

    init {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                syncModel.trySync()
                _updateImportUI.postValue(UpdateImportUI.FinishedSync)
            } catch (err: LbError) {
                isErrorVisible = true
                _updateImportUI.postValue(UpdateImportUI.NotifyError(err))
            }
        }
    }
}

sealed class UpdateImportUI {
    data class NotifyError(val error: LbError) : UpdateImportUI()
    object FinishedSync : UpdateImportUI()
}
