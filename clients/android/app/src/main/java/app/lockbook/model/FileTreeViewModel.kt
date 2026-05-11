package app.lockbook.model

import android.app.Application
import androidx.core.content.edit
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.preference.PreferenceManager
import app.lockbook.R
import app.lockbook.screen.UpdateFilesUI
import app.lockbook.ui.BreadCrumbItem
import app.lockbook.util.*
import com.afollestad.recyclical.datasource.emptyDataSourceTyped
import net.lockbook.File
import net.lockbook.Lb
import net.lockbook.LbEvent
import net.lockbook.LbStatus
import net.lockbook.Usage
import java.util.UUID

class FileTreeViewModel(application: Application) : AndroidViewModel(application) {

    internal val _notifyUpdateFilesUI = SingleMutableLiveData<UpdateFilesUI>()
    val notifyUpdateFilesUI: LiveData<UpdateFilesUI>
        get() = _notifyUpdateFilesUI

    lateinit var fileModel: FileModel

    // / the list of files that the file tree displays in the UI
    val _files = MutableLiveData<List<FileViewHolderInfo>>(emptyList())
    val files: LiveData<List<FileViewHolderInfo>>
        get() = _files

    val suggestedDocs = emptyDataSourceTyped<SuggestedDocsViewHolderInfo>()

    // / the current file path
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

    val _isSuggestedDocsVisible = MutableLiveData(true)
    val isSuggestedDocsVisible: LiveData<Boolean>
        get() = _isSuggestedDocsVisible

    val _usage = MutableLiveData<Usage?>()
    val usage: LiveData<Usage?>
        get() = _usage

    val _syncStatus = MutableLiveData<String?>()
    val syncStatus: LiveData<String?>
        get() = _syncStatus

    val _isSyncing = MutableLiveData(false)
    val isSyncing: LiveData<Boolean>
        get() = _isSyncing

    init {
        startUpInRoot()
        getStatus()
    }

    private fun getStatus() {
        val status: LbStatus = Lb.getStatus()
        hydrateStatusUpdate(status, null)
    }

    private fun checkUsage() {

        if (usage.value == null || _usage.value?.serverUsage == null) {
            return
        }

        val dataCap = usage.value?.dataCap?.exact?.toFloat() ?: 1f
        val serverUsage = usage.value?.serverUsage?.exact?.toFloat() ?: 1f

        val usageRatio = serverUsage / dataCap

        val pref = PreferenceManager.getDefaultSharedPreferences(getContext())

        val showOutOfSpace0_9 = pref.getBoolean(getString(R.string.show_running_out_of_space_0_9_key), true)
        val showOutOfSpace0_8 = pref.getBoolean(getString(R.string.show_running_out_of_space_0_8_key), true)

        when {
            usageRatio >= 1.0 -> {}
            usageRatio > 0.9 -> {
                if (!showOutOfSpace0_9) {
                    return
                }
            }
            usageRatio > 0.8 -> {
                if (!showOutOfSpace0_8) {
                    return
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

                return
            }
        }

        _notifyUpdateFilesUI.postValue(UpdateFilesUI.OutOfSpace((usageRatio * 100).toInt(), 100))
    }

    private fun startUpInRoot() {
        fileModel = FileModel.createAtRoot()

        refreshSuggestedDocs()
        refreshVisibleFiles()

        _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpdateBreadcrumbBar)
    }

    // / when you pass in null, it will return the parent of the current folder
    fun enterFolder(newParent: File?) {
        val parent = newParent ?: fileModel.idsAndFiles[fileModel.parent.parent] ?: fileModel.root
        fileModel.enterFolder(parent)

        updateSuggestedDocsVisibility()
        refreshVisibleFiles()

        _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpdateBreadcrumbBar)
    }

    fun reloadFiles() {
        fileModel.refreshFiles()

        refreshSuggestedDocs()
        refreshVisibleFiles()

        _notifyUpdateFilesUI.value = UpdateFilesUI.ToggleMenuBar
    }

    fun maybeToggleSuggestedDocs() {
        updateSuggestedDocsVisibility()
    }

    fun hydrateStatusUpdate(status: LbStatus, lbEvent: LbEvent?) {
        if (status.syncStatus != null) {
            _syncStatus.value = status.syncStatus
        }
        _isSyncing.value = status.syncing

        _dirtyLocally.value = status.dirtyLocally.orEmpty().mapNotNull { UUID.fromString(it) }.toHashSet()
        _pullingFiles.value = status.pullingFiles.orEmpty().mapNotNull { UUID.fromString(it) }.toHashSet()
        _pushingFiles.value = status.pushingFiles.orEmpty().mapNotNull { UUID.fromString(it) }.toHashSet()

        val metaOrContentDirty = if (lbEvent == null) { true } else { lbEvent.metadataChanged || lbEvent.pendingSharesChanged || lbEvent.documentWritten }

        if (metaOrContentDirty) {
            fileModel.refreshFiles()
        }

        refreshSuggestedDocs()
        refreshVisibleFiles()

        _usage.value = status.spaceUsed
        checkUsage()
    }

    fun clearSuggestedDocs() {
        fileModel.suggestedDocs = emptyList()
        suggestedDocs.clear()
        updateSuggestedDocsVisibility()
    }

    private fun refreshVisibleFiles() {
        _files.value = fileModel.children.intoViewHolderInfo(dirtyLocally.value, pullingFiles.value)
    }

    private fun refreshSuggestedDocs() {
        suggestedDocs.set(fileModel.suggestedDocs.intoSuggestedViewHolderInfo(fileModel.idsAndFiles))
        updateSuggestedDocsVisibility()
    }

    private fun updateSuggestedDocsVisibility() {
        _isSuggestedDocsVisible.value = fileModel.parent.isRoot && fileModel.suggestedDocs.isNotEmpty()
    }
}
