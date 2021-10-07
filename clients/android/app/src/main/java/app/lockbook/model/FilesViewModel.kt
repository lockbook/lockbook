package app.lockbook.model

import android.app.Application
import android.content.res.Resources
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.lockbook.App
import app.lockbook.R
import app.lockbook.screen.UpdateFilesUI
import app.lockbook.ui.BreadCrumbItem
import app.lockbook.util.ClientFileMetadata
import app.lockbook.util.LbError
import app.lockbook.util.SingleMutableLiveData
import com.afollestad.recyclical.datasource.SelectableDataSource
import com.afollestad.recyclical.datasource.selectableDataSourceOf
import com.afollestad.recyclical.datasource.selectableDataSourceTypedOf
import com.github.michaelbull.result.Err
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import java.io.File

class FilesViewModel(application: Application) : AndroidViewModel(application) {

    private val _notifyUpdateFilesUI = SingleMutableLiveData<UpdateFilesUI>()

    val notifyUpdateFilesUI: LiveData<UpdateFilesUI>
        get() = _notifyUpdateFilesUI

    lateinit var fileModel: FileModel
    var selectableFiles = selectableDataSourceTypedOf<ClientFileMetadata>()
    val syncModel = SyncModel(_notifyUpdateFilesUI)
    val shareModel = ShareModel(_notifyUpdateFilesUI)

    private fun postUIUpdate(update: UpdateFilesUI) {
        _notifyUpdateFilesUI.postValue(update)
    }

    fun enterFolder(folder: ClientFileMetadata) {
        viewModelScope.launch(Dispatchers.IO) {
            val intoChildResult = fileModel.intoChild(folder)
            if (intoChildResult is Err) {
                postUIUpdate(UpdateFilesUI.NotifyError((intoChildResult.error.toLbError(getRes()))))
                return@launch
            }

            selectableFiles.set(fileModel.children)
            postUIUpdate(UpdateFilesUI.UpdateBreadcrumbBar(fileModel.fileDir.map { BreadCrumbItem(it.name) }))
        }
    }

    fun onSwipeToRefresh() {
        viewModelScope.launch(Dispatchers.IO) {
            syncModel.trySync(getContext())
            refreshFiles()
            postUIUpdate(UpdateFilesUI.StopProgressSpinner)
        }
    }

    private fun refreshFiles() {
        val refreshChildrenResult = fileModel.refreshChildren()
        if (refreshChildrenResult is Err) {
            postUIUpdate(UpdateFilesUI.NotifyError(refreshChildrenResult.error.toLbError(getRes())))
            return
        }

        selectableFiles.set(fileModel.children)
    }

    fun shareSelectedFiles(appDataDir: File) {
        viewModelScope.launch(Dispatchers.IO) {
            shareModel.shareDocuments(selectableFiles.getSelectedItems(), appDataDir)
        }
    }

    fun deleteSelectedFiles() {
        viewModelScope.launch(Dispatchers.IO) {
            for(fileMetadata in selectableFiles.getSelectedItems()) {
                val deleteFileResult = CoreModel.deleteFile(App.config, fileMetadata.id)
                if(deleteFileResult is Err) {
                    _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(deleteFileResult.error.toLbError(getRes())))
                    return@launch
                }
            }
        }
    }

}