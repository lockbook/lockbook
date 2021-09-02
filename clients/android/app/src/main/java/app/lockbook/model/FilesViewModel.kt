package app.lockbook.model

import android.content.res.Resources
import androidx.lifecycle.LiveData
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.lockbook.ui.BreadCrumbItem
import app.lockbook.util.ClientFileMetadata
import app.lockbook.util.LbError
import app.lockbook.util.SingleMutableLiveData
import com.github.michaelbull.result.Err
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import java.io.File

class FilesViewModel: ViewModel() {

    private val _notifyUpdateFilesUI = SingleMutableLiveData<UpdateFilesUI>()

    val notifyUpdateFilesUI: LiveData<UpdateFilesUI>
        get() = _notifyUpdateFilesUI

    lateinit var fileModel: FileModel
    val syncModel = SyncModel(_notifyUpdateFilesUI)

    private fun postUIUpdate(update: UpdateFilesUI) {
        _notifyUpdateFilesUI.postValue(update)
    }

    fun enterFolder(folder: ClientFileMetadata, res: Resources) {
        viewModelScope.launch(Dispatchers.IO) {
            val intoChildResult = fileModel.intoChild(file)
            if (intoChildResult is Err) {
                postUIUpdate(UpdateFilesUI.NotifyError((intoChildResult.error.toLbError(getRes()))))
                return
            }
            postUIUpdate(UpdateFilesUI.UpdateFiles(fileModel.children))
            postUIUpdate(UpdateFilesUI.UpdateBreadcrumbBar(fileModel.fileDir.map { BreadCrumbItem(it.name) }))

        }
    }
}

sealed class UpdateFilesUI {
    data class UpdateBreadcrumbBar(val breadcrumbItems: List<BreadCrumbItem>): UpdateFilesUI()
    data class UpdateFiles(val files: List<ClientFileMetadata>): UpdateFilesUI()
    data class NotifyError(val error: LbError): UpdateFilesUI()
    object ShowSyncSnackBar: UpdateFilesUI()
    data class UpdateSyncSnackBar(val total: Int, val progress: Int): UpdateFilesUI()
    data class NotifyWithSnackbar(val msg: String): UpdateFilesUI()
}