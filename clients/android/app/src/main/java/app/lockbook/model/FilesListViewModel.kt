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
    val suggestedDocs = emptyDataSourceTyped<SuggestedDocsViewHolderInfo>()

    var breadcrumbItems = listOf<BreadCrumbItem>()

    var localChanges: HashSet<String> = hashSetOf()
    var serverChanges: HashSet<String>? = null
    var maybeLastSidebarInfo: UpdateFilesUI.UpdateSideBarInfo? = null

    var isSuggestedDocsVisible = true

    init {
        startUpInRoot()
        checkUsage()
    }

    private fun checkUsage() {
        viewModelScope.launch(Dispatchers.IO) {
            val usage = CoreModel.getUsage().getOrElse { error ->
                _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError((error.toLbError(getRes()))))
                return@launch
            }

            val usageRatio = usage.serverUsage.exact.toFloat() / usage.dataCap.exact

            val pref = PreferenceManager.getDefaultSharedPreferences(getContext())

            val showOutOfSpace0_9 = pref.getBoolean(getString(R.string.show_running_out_of_space_0_9_key), true)
            val showOutOfSpace0_8 = pref.getBoolean(getString(R.string.show_running_out_of_space_0_8_key), true)

            when {
                usageRatio >= 1.0 -> {}
                usageRatio > 0.9 -> {
                    if (!showOutOfSpace0_9) {
                        return@launch
                    }
                }
                usageRatio > 0.8 -> {
                    if (!showOutOfSpace0_8) {
                        return@launch
                    }
                }
                else -> {
                    if (!showOutOfSpace0_9) {
                        pref.edit()
                            .putBoolean(getString(R.string.show_running_out_of_space_0_9_key, true), true)
                            .apply()
                    }

                    if (!showOutOfSpace0_8) {
                        pref.edit()
                            .putBoolean(getString(R.string.show_running_out_of_space_0_8_key, true), true)
                            .apply()
                    }

                    return@launch
                }
            }

            _notifyUpdateFilesUI.postValue(UpdateFilesUI.OutOfSpace((usageRatio * 100).toInt(), 100))
        }
    }

    private fun startUpInRoot() {
        when (val createAtRootResult = FileModel.createAtRoot(getContext())) {
            is Ok -> {
                val tempFileModel = createAtRootResult.value
                if (tempFileModel == null) {
                    _notifyUpdateFilesUI.postValue(UpdateFilesUI.SyncImport)
                } else {
                    fileModel = tempFileModel

                    suggestedDocs.set(fileModel.suggestedDocs.intoSuggestedViewHolderInfo(fileModel.idsAndFiles))
                    files.set(fileModel.children.intoViewHolderInfo(localChanges, serverChanges))
                    breadcrumbItems = fileModel.fileDir.map { BreadCrumbItem(it.name) }

                    _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpdateBreadcrumbBar(breadcrumbItems))

                    viewModelScope.launch(Dispatchers.IO) {
                        maybeToggleSuggestedDocs()
                    }
                    refreshWorkInfo()
                }
            }
            is Err -> {
                _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(createAtRootResult.error))
            }
        }
    }

    private suspend fun maybeToggleSuggestedDocs() {
        val newIsSuggestedDocsVisible = fileModel.parent.parent == fileModel.parent.id && !suggestedDocs.isEmpty()
        if (newIsSuggestedDocsVisible != isSuggestedDocsVisible) {
            isSuggestedDocsVisible = newIsSuggestedDocsVisible
            withContext(Dispatchers.Main) {
                _notifyUpdateFilesUI.value = UpdateFilesUI.ToggleSuggestedDocsVisibility(isSuggestedDocsVisible)
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
//                        activityModel.launchDetailScreen(DetailScreen.Loading(createFileResult.value))
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

            maybeToggleSuggestedDocs()

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
            maybeToggleSuggestedDocs()

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
            maybeToggleSuggestedDocs()
            breadcrumbItems = fileModel.fileDir.map { BreadCrumbItem(it.name) }
            viewModelScope.launch(Dispatchers.Main) {
                files.set(fileModel.children.intoViewHolderInfo(localChanges, serverChanges))
            }

            _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpdateBreadcrumbBar(breadcrumbItems))
        }
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

    fun fileOpened(id: String) {
        if (fileModel.verifyOpenFile(id)) {
            viewModelScope.launch(Dispatchers.Main) {
                files.set(fileModel.children.intoViewHolderInfo(localChanges, serverChanges))
            }

            breadcrumbItems = fileModel.fileDir.map { BreadCrumbItem(it.name) }
            _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpdateBreadcrumbBar(breadcrumbItems))
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
            suggestedDocs.set(fileModel.suggestedDocs.intoSuggestedViewHolderInfo(fileModel.idsAndFiles))

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
                    calculateWorkResult.value.latestServerTS
                )
                sidebarInfo.serverDirtyFilesCount = calculateWorkResult.value.workUnits.filter { it.tag == WorkUnitTag.ServerChange }.size

                serverChanges = calculateWorkResult.value.workUnits.filter { it.tag == WorkUnitTag.ServerChange }.map { it.content }.toHashSet()
                viewModelScope.launch(Dispatchers.Main) {
                    files.set(fileModel.children.intoViewHolderInfo(localChanges, serverChanges))
                }
            }
            is Err -> if ((calculateWorkResult.error as? CoreError.UiError)?.content != CalculateWorkError.CouldNotReachServer) {
                _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(calculateWorkResult.error.toLbError(getRes())))
            }
        }

        _notifyUpdateFilesUI.postValue(sidebarInfo)

        when (val pendingSharesResult = CoreModel.getPendingShares()) {
            is Ok -> sidebarInfo.hasPendingShares = pendingSharesResult.value.isNotEmpty()
            is Err -> _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(pendingSharesResult.error.toLbError(getRes())))
        }

        _notifyUpdateFilesUI.postValue(sidebarInfo)
    }
}
