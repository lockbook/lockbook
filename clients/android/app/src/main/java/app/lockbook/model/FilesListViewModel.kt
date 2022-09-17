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
import com.afollestad.recyclical.datasource.emptyDataSourceTyped
import com.afollestad.recyclical.datasource.emptySelectableDataSourceTyped
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.getOrElse
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class FilesListViewModel(application: Application) : AndroidViewModel(application) {

    private val _notifyUpdateFilesUI = SingleMutableLiveData<UpdateFilesUI>()

    val notifyUpdateFilesUI: LiveData<UpdateFilesUI>
        get() = _notifyUpdateFilesUI

    lateinit var fileModel: FileModel

    val files = emptySelectableDataSourceTyped<FileViewHolderInfo>()
    val recentFiles = emptyDataSourceTyped<RecentFileViewHolderInfo>()

    var breadcrumbItems = listOf<BreadCrumbItem>()

    val syncModel = SyncModel()
    var localChanges: HashSet<String> = hashSetOf()
    var serverChanges: HashSet<String>? = null
    var maybeLastSidebarInfo: UpdateFilesUI.UpdateSideBarInfo? = null

    var isRecentFilesVisible = true

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

                    recentFiles.set(fileModel.recentFiles.intoRecentViewHolderInfo(fileModel.files))
                    files.set(fileModel.children.intoViewHolderInfo(localChanges, serverChanges))
                    breadcrumbItems = fileModel.fileDir.map { BreadCrumbItem(it.name) }

                    _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpdateBreadcrumbBar(breadcrumbItems))

                    refreshWorkInfo()
                }
            }
            is Err -> {
                _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(createAtRootResult.error))
            }
        }
    }

    private suspend fun maybeToggleRecentFiles() {
        val newIsRecentFilesVisible = fileModel.parent.parent == fileModel.parent.id
        if (newIsRecentFilesVisible != isRecentFilesVisible) {
            isRecentFilesVisible = newIsRecentFilesVisible
            withContext(Dispatchers.Main) {
                _notifyUpdateFilesUI.value = UpdateFilesUI.ToggleRecentFilesVisibility(isRecentFilesVisible)
            }
        }
    }

    fun generateQuickNote(activityModel: StateViewModel) {
        viewModelScope.launch(Dispatchers.IO) {
            var iter = 0
            var fileName: String

            do {
                iter++
                fileName = "${getString(R.string.note)}-$iter.md"
            } while (fileModel.children.any { it.name == fileName })

            when (val createFileResult = CoreModel.createFile(fileModel.parent.id, fileName, FileType.Document)) {
                is Ok -> {
                    withContext(Dispatchers.Main) {
                        activityModel.launchDetailsScreen(DetailScreen.Loading(createFileResult.value))
                    }

                    refreshFiles()
                }
                is Err -> _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(createFileResult.error.toLbError(getRes())))
            }
        }
    }

    fun enterFolder(folder: File) {
        viewModelScope.launch(Dispatchers.IO) {
            fileModel.intoFile(folder)

            maybeToggleRecentFiles()

            localChanges = CoreModel.getLocalChanges().getOrElse { error ->
                _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError((error.toLbError(getRes()))))
                return@launch
            }

            viewModelScope.launch(Dispatchers.Main) {
                files.set(fileModel.children.intoViewHolderInfo(localChanges, serverChanges))
            }

            breadcrumbItems = fileModel.fileDir.map { BreadCrumbItem(it.name) }
            _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpdateBreadcrumbBar(breadcrumbItems))
        }
    }

    fun intoParentFolder() {
        viewModelScope.launch(Dispatchers.IO) {
            fileModel.intoParent()
            maybeToggleRecentFiles()

            viewModelScope.launch(Dispatchers.Main) {
                files.set(fileModel.children.intoViewHolderInfo(localChanges, serverChanges))
            }

            breadcrumbItems = fileModel.fileDir.map { BreadCrumbItem(it.name) }
            _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpdateBreadcrumbBar(breadcrumbItems))
        }
    }

    fun intoAncestralFolder(position: Int) {
        viewModelScope.launch(Dispatchers.IO) {
            fileModel.refreshChildrenAtAncestor(position)
            maybeToggleRecentFiles()
            breadcrumbItems = fileModel.fileDir.map { BreadCrumbItem(it.name) }
            viewModelScope.launch(Dispatchers.Main) {
                files.set(fileModel.children.intoViewHolderInfo(localChanges, serverChanges))
            }

            _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpdateBreadcrumbBar(breadcrumbItems))
        }
    }

    fun onSwipeToRefresh() {
        viewModelScope.launch(Dispatchers.IO) {
            sync()
            refreshFiles()
            refreshWorkInfo()
        }
    }

    fun sync(usePreferences: Boolean) {
        viewModelScope.launch(Dispatchers.IO) {
            if (!usePreferences || PreferenceManager.getDefaultSharedPreferences(getContext())
                .getBoolean(
                        getString(
                                getRes(),
                                R.string.sync_automatically_key
                            ),
                        false
                    )
            ) {
                sync()
            }
            refreshFiles()
        }
    }

    private fun sync() {
        serverChanges = null
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

    fun reloadWorkInfo() {
        viewModelScope.launch(Dispatchers.IO) {
            refreshWorkInfo()
        }
    }

    private fun refreshFiles() {
        val refreshChildrenResult = fileModel.refreshFiles()
        if (refreshChildrenResult is Err) {
            _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(refreshChildrenResult.error.toLbError(getRes())))
            return
        }

        localChanges = CoreModel.getLocalChanges().getOrElse { error ->
            _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError((error.toLbError(getRes()))))
            return
        }

        viewModelScope.launch(Dispatchers.Main) {
            files.deselectAll()
            files.set(fileModel.children.intoViewHolderInfo(localChanges, serverChanges))
            recentFiles.set(fileModel.recentFiles.intoRecentViewHolderInfo(fileModel.files))

            _notifyUpdateFilesUI.value = UpdateFilesUI.ToggleMenuBar
        }
    }

    private fun refreshWorkInfo() {
        val sidebarInfo = UpdateFilesUI.UpdateSideBarInfo()
        maybeLastSidebarInfo = sidebarInfo

        when (val usageResult = CoreModel.getUsage()) {
            is Ok -> sidebarInfo.usageMetrics = usageResult.value
            is Err -> if ((usageResult.error as? CoreError.UiError)?.content != GetUsageError.CouldNotReachServer) {
                _notifyUpdateFilesUI.postValue(
                    UpdateFilesUI.NotifyError(usageResult.error.toLbError(getRes()))
                )
            }
        }

        _notifyUpdateFilesUI.postValue(sidebarInfo)

        when (val getLocalChangesResult = CoreModel.getLocalChanges()) {
            is Ok -> sidebarInfo.localDirtyFilesCount = getLocalChangesResult.value.size
            is Err -> {
                return _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(getLocalChangesResult.error.toLbError(getRes())))
            }
        }

        _notifyUpdateFilesUI.postValue(sidebarInfo)

        when (val calculateWorkResult = CoreModel.calculateWork()) {
            is Ok -> {
                sidebarInfo.lastSynced = CoreModel.convertToHumanDuration(
                    calculateWorkResult.value.mostRecentUpdateFromServer
                )
                sidebarInfo.serverDirtyFilesCount = calculateWorkResult.value.workUnits.filter { it.tag == WorkUnitTag.ServerChange }.size

                serverChanges = calculateWorkResult.value.workUnits.filter { it.tag == WorkUnitTag.ServerChange }.map { it.content.metadata.id }.toHashSet()
                viewModelScope.launch(Dispatchers.Main) {
                    files.set(fileModel.children.intoViewHolderInfo(localChanges, serverChanges))
                }
            }
            is Err -> if ((calculateWorkResult.error as? CoreError.UiError)?.content != CalculateWorkError.CouldNotReachServer) {
                _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(calculateWorkResult.error.toLbError(getRes())))
            }
        }

        _notifyUpdateFilesUI.postValue(sidebarInfo)
    }
}
