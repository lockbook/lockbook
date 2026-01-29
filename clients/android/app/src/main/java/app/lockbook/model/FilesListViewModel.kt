package app.lockbook.model

import android.app.Application
import androidx.core.content.edit
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
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.lockbook.File
import net.lockbook.Lb
import net.lockbook.LbError
import net.lockbook.LbError.LbEC

class FilesListViewModel(application: Application) : AndroidViewModel(application) {

    internal val _notifyUpdateFilesUI = SingleMutableLiveData<UpdateFilesUI>()
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
            val usage = try {
                Lb.getUsage()
            } catch (err: LbError) {
                _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError((err)))
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
                        pref.edit {
                            putBoolean(getString(R.string.show_running_out_of_space_0_9_key, true), true)
                        }
                    }

                    if (!showOutOfSpace0_8) {
                        pref.edit {
                            putBoolean(getString(R.string.show_running_out_of_space_0_8_key, true), true)
                        }
                    }

                    return@launch
                }
            }

            _notifyUpdateFilesUI.postValue(UpdateFilesUI.OutOfSpace((usageRatio * 100).toInt(), 100))
        }
    }

    private fun startUpInRoot() {
        try {
            fileModel = FileModel.createAtRoot()
            suggestedDocs.set(fileModel.suggestedDocs.intoSuggestedViewHolderInfo(fileModel.idsAndFiles))
            files.set(fileModel.children.intoViewHolderInfo(localChanges, serverChanges))
            breadcrumbItems = fileModel.fileDir.map { BreadCrumbItem(it.name) }

            _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpdateBreadcrumbBar(breadcrumbItems))

            viewModelScope.launch(Dispatchers.IO) {
                maybeToggleSuggestedDocs()
                refreshWorkInfo()
            }
        } catch (err: LbError) {
            if (err.kind == LbEC.RootNonexistent) {
                _notifyUpdateFilesUI.postValue(UpdateFilesUI.SyncImport)
            } else {
                _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(err))
            }
        }
    }

    suspend fun maybeToggleSuggestedDocs() {
        val newIsSuggestedDocsVisible = fileModel.parent.parent == fileModel.parent.id && !suggestedDocs.isEmpty()

        if (newIsSuggestedDocsVisible != isSuggestedDocsVisible) {
            isSuggestedDocsVisible = newIsSuggestedDocsVisible
            withContext(Dispatchers.Main) {
                _notifyUpdateFilesUI.value = UpdateFilesUI.ToggleSuggestedDocsVisibility(isSuggestedDocsVisible)
            }
        }
    }

    fun enterFolder(folder: File) {
        viewModelScope.launch(Dispatchers.IO) {
            fileModel.intoFile(folder)

            maybeToggleSuggestedDocs()

            try {
                localChanges = Lb.getLocalChanges().toHashSet()
            } catch (err: LbError) {
                _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(err))
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
        try {
            if (fileModel.verifyOpenFile(id)) {
                viewModelScope.launch(Dispatchers.Main) {
                    files.set(fileModel.children.intoViewHolderInfo(localChanges, serverChanges))
                }

                breadcrumbItems = fileModel.fileDir.map { BreadCrumbItem(it.name) }
                _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpdateBreadcrumbBar(breadcrumbItems))
            }
        } catch (err: LbError) {
            _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(err))
        }
    }

    private fun refreshFiles() {
        try {
            fileModel.refreshFiles()
            localChanges = Lb.getLocalChanges().toHashSet()

            viewModelScope.launch(Dispatchers.Main) {
                files.deselectAll()
                files.set(fileModel.children.intoViewHolderInfo(localChanges, serverChanges))
                suggestedDocs.set(fileModel.suggestedDocs.intoSuggestedViewHolderInfo(fileModel.idsAndFiles))

                _notifyUpdateFilesUI.value = UpdateFilesUI.ToggleMenuBar
            }
        } catch (err: LbError) {
            _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(err))
        }
    }

    private fun refreshWorkInfo() {
        val sidebarInfo = UpdateFilesUI.UpdateSideBarInfo()
        maybeLastSidebarInfo = sidebarInfo

        try {
            sidebarInfo.usageMetrics = Lb.getUsage()
        } catch (err: LbError) {
            if (err.kind != LbEC.ServerUnreachable) {
                _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(err))
            }
        }

        _notifyUpdateFilesUI.postValue(sidebarInfo)

        try {
            sidebarInfo.localDirtyFilesCount = Lb.getLocalChanges().size
        } catch (err: LbError) {
            return _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(err))
        }

        _notifyUpdateFilesUI.postValue(sidebarInfo)

        try {
            val syncWork = Lb.calculateWork()
            sidebarInfo.lastSynced = Lb.getTimestampHumanString(syncWork.latestServerTS)
            sidebarInfo.serverDirtyFilesCount = syncWork.workUnits.filter { !it.isLocalChange }.size

            serverChanges = syncWork.workUnits.filter { !it.isLocalChange }.map { it.id }.toHashSet()
            viewModelScope.launch(Dispatchers.Main) {
                files.set(fileModel.children.intoViewHolderInfo(localChanges, serverChanges))
            }
        } catch (err: LbError) {
            if (err.kind != LbEC.ServerUnreachable) {
                _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(err))
            }
        }

        _notifyUpdateFilesUI.postValue(sidebarInfo)

        try {
            sidebarInfo.hasPendingShares = Lb.getPendingShares().isNotEmpty()
        } catch (err: LbError) {
            _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(err))
        }

        _notifyUpdateFilesUI.postValue(sidebarInfo)
    }
}
