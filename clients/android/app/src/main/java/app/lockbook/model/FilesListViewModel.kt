package app.lockbook.model

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.viewModelScope
import androidx.preference.PreferenceManager
import app.lockbook.*
import app.lockbook.screen.UpdateFilesUI
import app.lockbook.ui.BreadCrumbItem
import app.lockbook.util.DecryptedFileMetadata
import app.lockbook.util.SingleMutableLiveData
import app.lockbook.util.exhaustive
import com.afollestad.recyclical.datasource.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

class FilesListViewModel(application: Application, isThisANewAccount: Boolean) : AndroidViewModel(application) {

    private val _notifyUpdateFilesUI = SingleMutableLiveData<UpdateFilesUI>()

    val notifyUpdateFilesUI: LiveData<UpdateFilesUI>
        get() = _notifyUpdateFilesUI

    lateinit var fileModel: FileModel

    val selectableFiles = emptySelectableDataSourceTyped<DecryptedFileMetadata>()
    var breadcrumbItems = listOf<BreadCrumbItem>()

    val syncModel = SyncModel()

    init {
        viewModelScope.launch(Dispatchers.IO) {
            if (isThisANewAccount) {
                _notifyUpdateFilesUI.postValue(UpdateFilesUI.ShowBeforeWeStart)
            }

            startUpInRoot()
        }
    }

    private fun startUpInRoot() {
        when (val createAtRootResult = FileModel.createAtRoot(getContext())) {
            is Ok -> {
                fileModel = createAtRootResult.value
                refreshFiles()
                breadcrumbItems = fileModel.fileDir.map { BreadCrumbItem(it.decryptedName) }
                _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpdateBreadcrumbBar(breadcrumbItems))
            }
            is Err -> _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(createAtRootResult.error))
        }
    }

    private fun postUIUpdate(update: UpdateFilesUI) {
        _notifyUpdateFilesUI.postValue(update)
    }

    fun enterFolder(folder: DecryptedFileMetadata) {
        viewModelScope.launch(Dispatchers.IO) {
            val intoChildResult = fileModel.intoChild(folder)
            if (intoChildResult is Err) {
                postUIUpdate(UpdateFilesUI.NotifyError((intoChildResult.error.toLbError(getRes()))))
                return@launch
            }

            viewModelScope.launch(Dispatchers.Main) {
                selectableFiles.set(fileModel.children)
            }

            breadcrumbItems = fileModel.fileDir.map { BreadCrumbItem(it.decryptedName) }
            _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpdateBreadcrumbBar(breadcrumbItems))
        }
    }

    fun intoParentFolder() {
        viewModelScope.launch(Dispatchers.IO) {
            val intoParentResult = fileModel.intoParent()
            if (intoParentResult is Err) {
                postUIUpdate(UpdateFilesUI.NotifyError((intoParentResult.error.toLbError(getRes()))))
                return@launch
            }

            viewModelScope.launch(Dispatchers.Main) {
                selectableFiles.set(fileModel.children)
            }

            breadcrumbItems = fileModel.fileDir.map { BreadCrumbItem(it.decryptedName) }
            _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpdateBreadcrumbBar(breadcrumbItems))
        }
    }

    fun intoAncestralFolder(position: Int) {
        viewModelScope.launch(Dispatchers.IO) {
            val intoAncestorResult = fileModel.refreshChildrenAtAncestor(position)
            if (intoAncestorResult is Err) {
                postUIUpdate(UpdateFilesUI.NotifyError((intoAncestorResult.error.toLbError(getRes()))))
                return@launch
            }

            viewModelScope.launch(Dispatchers.Main) {
                selectableFiles.set(fileModel.children)
            }

            breadcrumbItems = fileModel.fileDir.map { BreadCrumbItem(it.decryptedName) }

            _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpdateBreadcrumbBar(breadcrumbItems))
        }
    }

    fun onSwipeToRefresh() {
        viewModelScope.launch(Dispatchers.IO) {
            syncWithSnackBar()
            refreshFiles()
            postUIUpdate(UpdateFilesUI.StopProgressSpinner)
        }
    }

    fun syncBasedOnPreferences() {
        viewModelScope.launch(Dispatchers.IO) {
            if (PreferenceManager.getDefaultSharedPreferences(getContext())
                .getBoolean(
                        app.lockbook.util.getString(
                                getRes(),
                                R.string.sync_automatically_key
                            ),
                        false
                    )
            ) {
                syncWithSnackBar()
            }
            refreshFiles()
        }
    }

    private fun syncWithSnackBar() {
        when (val hasSyncWorkResult = syncModel.hasSyncWork()) {
            is Ok -> if (!hasSyncWorkResult.value) {
                _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyWithSnackbar(getString(R.string.list_files_sync_finished_snackbar)))
                return
            }
            is Err -> _notifyUpdateFilesUI.postValue(
                UpdateFilesUI.NotifyError(
                    hasSyncWorkResult.error.toLbError(
                        getRes()
                    )
                )
            )
        }

        when (val syncResult = syncModel.trySync()) {
            is Ok -> _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyWithSnackbar(getString(R.string.list_files_sync_finished_snackbar)))
            is Err -> _notifyUpdateFilesUI.postValue(
                UpdateFilesUI.NotifyError(
                    syncResult.error.toLbError(
                        getRes()
                    )
                )
            )
        }.exhaustive
    }

    fun reloadFiles() {
        viewModelScope.launch(Dispatchers.IO) {
            refreshFiles()
        }
    }

    private fun refreshFiles() {
        val refreshChildrenResult = fileModel.refreshChildren()
        if (refreshChildrenResult is Err) {
            postUIUpdate(UpdateFilesUI.NotifyError(refreshChildrenResult.error.toLbError(getRes())))
            return
        }

        viewModelScope.launch(Dispatchers.Main) {
            selectableFiles.deselectAll()
            selectableFiles.set(fileModel.children)
        }

        _notifyUpdateFilesUI.postValue(UpdateFilesUI.ToggleMenuBar)
    }

    fun deleteSelectedFiles() {
        viewModelScope.launch(Dispatchers.IO) {
            for (fileMetadata in selectableFiles.getSelectedItems()) {
                val deleteFileResult = CoreModel.deleteFile(App.config, fileMetadata.id)
                if (deleteFileResult is Err) {
                    _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(deleteFileResult.error.toLbError(getRes())))
                    return@launch
                }
            }

            refreshFiles()
        }
    }

    fun changeFileSort(newSortStyle: SortStyle) {
        fileModel.setSortStyle(newSortStyle)
        selectableFiles.set(fileModel.children)

        PreferenceManager.getDefaultSharedPreferences(getContext()).edit().putString(getString(R.string.sort_files_key), getString(newSortStyle.toStringResource())).apply()
    }
}
