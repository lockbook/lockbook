package app.lockbook.model

import android.app.Application
import androidx.core.content.edit
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import androidx.preference.PreferenceManager
import app.lockbook.R
import app.lockbook.screen.UpdateFilesUI
import app.lockbook.ui.BreadCrumbItem
import app.lockbook.util.*
import app.lockbook.workspace.LbStatus
import app.lockbook.workspace.SpaceUsed
import com.afollestad.recyclical.datasource.emptyDataSourceTyped
import com.afollestad.recyclical.datasource.emptySelectableDataSourceTyped
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import net.lockbook.File
import net.lockbook.Lb
import net.lockbook.LbError
import net.lockbook.LbError.LbEC
import net.lockbook.Usage
import java.util.UUID

class FileTreeViewModel(application: Application) : AndroidViewModel(application) {

    internal val _notifyUpdateFilesUI = SingleMutableLiveData<UpdateFilesUI>()
    val notifyUpdateFilesUI: LiveData<UpdateFilesUI>
        get() = _notifyUpdateFilesUI

    lateinit var fileModel: FileModel

    /// the list of files that the file tree displays in the UI
    val files = emptySelectableDataSourceTyped<FileViewHolderInfo>()

    val suggestedDocs = emptyDataSourceTyped<SuggestedDocsViewHolderInfo>()

    /// the current file path
    val _breadcrumbItems = MutableLiveData<MutableList<BreadCrumbItem>>()
    val breadcrumbItems: LiveData<MutableList<BreadCrumbItem>>
        get() = _breadcrumbItems


    val _dirtyLocally = MutableLiveData<Set<UUID>>()
    val dirtyLocally: LiveData<Set<UUID>>
        get() = _dirtyLocally

    val _pullingFiles = MutableLiveData<Set<UUID>>()
    val pullingFiles: LiveData<Set<UUID>>
        get() = _pullingFiles

    val _pushingFiles = MutableLiveData<Set<UUID>>()
    val pushingFiles: LiveData<Set<UUID>>
        get() = _pushingFiles

    var maybeLastSidebarInfo: UpdateFilesUI.UpdateSideBarInfo? = null

    val _isSuggestedDocsVisible = MutableLiveData(true)
    val isSuggestedDocsVisible: LiveData<Boolean>
        get() = _isSuggestedDocsVisible

    val _usage = MutableLiveData<SpaceUsed?>()
    val usage: LiveData<SpaceUsed?>
        get() = _usage

    val _syncStatus = MutableLiveData<String?>()
    val syncStatus: LiveData<String?>
        get() = _syncStatus


    init {
        startUpInRoot()
        checkUsage()
    }

    // todo: use status.usage and populate this on status update. it will result in a better
    // running out of storage experience. currently we only check at startup
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
            files.set(fileModel.children.intoViewHolderInfo(dirtyLocally.value, pullingFiles.value))

            _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpdateBreadcrumbBar)

            viewModelScope.launch(Dispatchers.IO) {
                maybeToggleSuggestedDocs()
            }
        } catch (err: LbError) {
            if (err.kind == LbEC.RootNonexistent) {
                _notifyUpdateFilesUI.postValue(UpdateFilesUI.SyncImport)
            } else {
                _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(err))
            }
        }
    }

    fun maybeToggleSuggestedDocs() {
        _isSuggestedDocsVisible.value = fileModel.parent.isRoot && !suggestedDocs.isEmpty()
    }

    fun enterFolder(folder: File) {
        viewModelScope.launch(Dispatchers.IO) {
            fileModel.enterFolder(folder)

            maybeToggleSuggestedDocs()

            viewModelScope.launch(Dispatchers.Main) {
                files.set(fileModel.children.intoViewHolderInfo(dirtyLocally.value, pullingFiles.value))
            }

            _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpdateBreadcrumbBar)
        }
    }

    fun intoParentFolder() {
        viewModelScope.launch(Dispatchers.IO) {
            fileModel.intoParent()
            maybeToggleSuggestedDocs()

            viewModelScope.launch(Dispatchers.Main) {
                files.set(fileModel.children.intoViewHolderInfo(dirtyLocally.value, pullingFiles.value))
            }

            _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpdateBreadcrumbBar)
        }
    }

    fun intoAncestralFolder(newParent: File) {
        viewModelScope.launch(Dispatchers.IO) {
            fileModel.refreshChildrenAtAncestor(newParent)

            maybeToggleSuggestedDocs()
            viewModelScope.launch(Dispatchers.Main) {
                files.set(fileModel.children.intoViewHolderInfo(dirtyLocally.value, pullingFiles.value))
            }

            _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpdateBreadcrumbBar)
        }
    }

    fun reloadFiles() {
        viewModelScope.launch(Dispatchers.IO) {
            refreshFiles()
        }
    }

    private fun refreshFiles() {
        try {
            fileModel.refreshFiles()

            viewModelScope.launch(Dispatchers.Main) {
                files.deselectAll()
                files.set(fileModel.children.intoViewHolderInfo(dirtyLocally.value, pullingFiles.value))
                suggestedDocs.set(fileModel.suggestedDocs.intoSuggestedViewHolderInfo(fileModel.idsAndFiles))

                _notifyUpdateFilesUI.value = UpdateFilesUI.ToggleMenuBar
            }
        } catch (err: LbError) {
            _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(err))
        }
    }

    private fun refreshSidebarInfo() {
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

        sidebarInfo.localDirtyFilesCount = dirtyLocally.value?.size

        _notifyUpdateFilesUI.postValue(sidebarInfo)

        try {
            // val syncWork = Lb.calculateWork()
            // sidebarInfo.lastSynced = Lb.getTimestampHumanString(syncWork.latestServerTS)
            // sidebarInfo.serverDirtyFilesCount = syncWork.workUnits.filter { !it.isLocalChange }.size

            // serverChanges = syncWork.workUnits.filter { !it.isLocalChange }.map { it.id }.toHashSet()
            // viewModelScope.launch(Dispatchers.Main) {
            //    files.set(fileModel.children.intoViewHolderInfo(localChanges, serverChanges))
            // }
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

    fun hydrateLbStatus(status: LbStatus){
        _dirtyLocally.value = status.dirtyLocally.mapNotNull { UUID.fromString(it) }.toHashSet()
        _pullingFiles.value = status.pullingFiles.mapNotNull { UUID.fromString(it) }.toHashSet()
        _pushingFiles.value = status.pushingFiles.mapNotNull { UUID.fromString(it) }.toHashSet()

        _usage.value = status.spaceUsed

        _syncStatus.value = status.syncStatus

    }
}
