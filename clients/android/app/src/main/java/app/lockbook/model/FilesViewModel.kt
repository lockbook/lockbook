package app.lockbook.model

import android.app.Application
import android.content.res.Resources
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.preference.PreferenceManager
import app.lockbook.*
import app.lockbook.screen.UpdateFilesUI
import app.lockbook.ui.BreadCrumbItem
import app.lockbook.util.ClientFileMetadata
import app.lockbook.util.LbError
import app.lockbook.util.SingleMutableLiveData
import com.afollestad.recyclical.datasource.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import timber.log.Timber
import java.io.File

class FilesViewModel(application: Application) : AndroidViewModel(application) {

    private val _notifyUpdateFilesUI = SingleMutableLiveData<UpdateFilesUI>()

    val notifyUpdateFilesUI: LiveData<UpdateFilesUI>
        get() = _notifyUpdateFilesUI

    lateinit var fileModel: FileModel
    val selectableFiles = emptySelectableDataSourceTyped<ClientFileMetadata>()
    val syncModel = SyncModel(_notifyUpdateFilesUI)
    val shareModel = ShareModel(_notifyUpdateFilesUI)

    init {
        startUpInRoot()
    }

    private fun startUpInRoot() {
        when (val createAtRootResult = FileModel.createAtRoot(getContext())) {
            is Ok -> {
                fileModel = createAtRootResult.value
                refreshFiles()
                _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpdateBreadcrumbBar(fileModel.fileDir.map { BreadCrumbItem(it.name) }))
            }
            is Err -> _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(createAtRootResult.error))
        }
    }

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

            viewModelScope.launch(Dispatchers.Main) {
                selectableFiles.set(fileModel.children)
            }
            postUIUpdate(UpdateFilesUI.UpdateBreadcrumbBar(fileModel.fileDir.map { BreadCrumbItem(it.name) }))
        }
    }

    fun intoParentFolder() {
        viewModelScope.launch(Dispatchers.IO) {
            Timber.e("HERE AT MY NEW LABO")
            val intoParentResult = fileModel.intoParent()
            if (intoParentResult is Err) {
                postUIUpdate(UpdateFilesUI.NotifyError((intoParentResult.error.toLbError(getRes()))))
                return@launch
            }

            viewModelScope.launch(Dispatchers.Main) {
                selectableFiles.set(fileModel.children)
            }

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

        viewModelScope.launch(Dispatchers.Main) {
            selectableFiles.set(fileModel.children)
        }
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

    fun changeFileSort(newSortStyle: SortStyle) {
        fileModel.setSortStyle(newSortStyle)
        selectableFiles.set(fileModel.children)

        PreferenceManager.getDefaultSharedPreferences(getContext()).edit().putString(getString(R.string.sort_files_key), getString(newSortStyle.toStringResource()))
    }
}