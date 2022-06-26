package app.lockbook.model

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.viewModelScope
import androidx.preference.PreferenceManager
import app.lockbook.R
import app.lockbook.screen.UpdateFilesUI
import app.lockbook.ui.BreadCrumbItem
import app.lockbook.util.*
import com.afollestad.recyclical.datasource.emptySelectableDataSourceTyped
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

class FilesListViewModel(application: Application) : AndroidViewModel(application) {

    private val _notifyUpdateFilesUI = SingleMutableLiveData<UpdateFilesUI>()

    val notifyUpdateFilesUI: LiveData<UpdateFilesUI>
        get() = _notifyUpdateFilesUI

    lateinit var fileModel: FileModel

    val selectableFiles = emptySelectableDataSourceTyped<DecryptedFileMetadata>()
    var breadcrumbItems = listOf<BreadCrumbItem>()

    val syncModel = SyncModel()
    var maybeLastSidebarInfo: UpdateFilesUI.UpdateSideBarInfo? = null

    init {
        startUpInRoot()
    }

    private fun startUpInRoot() {
        when (val createAtRootResult = FileModel.createAtRoot(getContext())) {
            is Ok -> {
                val tempFileModel = createAtRootResult.value
                if (tempFileModel == null) {
                    _notifyUpdateFilesUI.postValue(UpdateFilesUI.SyncImport)
                } else {
                    fileModel = tempFileModel

                    refreshFiles()
                    breadcrumbItems = fileModel.fileDir.map { BreadCrumbItem(it.decryptedName) }
                    _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpdateBreadcrumbBar(breadcrumbItems))

                    refreshSidebar()
                }
            }
            is Err -> {
                _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(createAtRootResult.error))
            }
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
            refreshSidebar()
        }
    }

    fun syncBasedOnPreferences() {
        viewModelScope.launch(Dispatchers.IO) {
            if (PreferenceManager.getDefaultSharedPreferences(getContext())
                .getBoolean(
                        getString(
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
                _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpToDateSyncSnackBar)
                return
            }
            is Err -> return _notifyUpdateFilesUI.postValue(
                UpdateFilesUI.NotifyError(
                    hasSyncWorkResult.error.toLbError(
                        getRes()
                    )
                )
            )
        }

        when (val syncResult = syncModel.trySync()) {
            is Ok -> _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpToDateSyncSnackBar)
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

    fun reloadSidebar() {
        viewModelScope.launch(Dispatchers.IO) {
            refreshSidebar()
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

            _notifyUpdateFilesUI.value = UpdateFilesUI.ToggleMenuBar
        }
    }

    fun changeFileSort(newSortStyle: SortStyle) {
        fileModel.setSortStyle(newSortStyle)
        selectableFiles.set(fileModel.children)

        PreferenceManager.getDefaultSharedPreferences(getContext()).edit().putString(getString(R.string.sort_files_key), getString(newSortStyle.toStringResource())).apply()
    }

    private fun refreshSidebar() {
        var lastSidebarInfo = UpdateFilesUI.UpdateSideBarInfo()
        maybeLastSidebarInfo = lastSidebarInfo

        when (val usageResult = CoreModel.getUsage()) {
            is Ok -> lastSidebarInfo.usageMetrics = usageResult.value
            is Err -> return _notifyUpdateFilesUI.postValue(
                UpdateFilesUI.NotifyError(usageResult.error.toLbError(getRes()))
            )
        }

        _notifyUpdateFilesUI.postValue(lastSidebarInfo)

        when (val getLocalChangesResult = CoreModel.getLocalChanges()) {
            is Ok -> lastSidebarInfo.localDirtyFilesCount = getLocalChangesResult.value.size
            is Err -> {
                return _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(getLocalChangesResult.error.toLbError(getRes())))
            }
        }

        _notifyUpdateFilesUI.postValue(lastSidebarInfo)

        when (val calculateWorkResult = CoreModel.calculateWork()) {
            is Ok -> {
                lastSidebarInfo.lastSynced = CoreModel.convertToHumanDuration(
                    calculateWorkResult.value.mostRecentUpdateFromServer
                )
                lastSidebarInfo.serverDirtyFilesCount = calculateWorkResult.value.workUnits.filter { it.tag == WorkUnitTag.ServerChange }.size
            }
            is Err -> {
                _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(calculateWorkResult.error.toLbError(getRes())))
            }
        }

        _notifyUpdateFilesUI.postValue(lastSidebarInfo)
    }
}
