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
import kotlinx.coroutines.withContext
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.Json
import net.lockbook.File
import net.lockbook.Lb
import net.lockbook.LbError
import net.lockbook.LbEvent
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

    val _isSuggestedDocsVisible = MutableLiveData(true)
    val isSuggestedDocsVisible: LiveData<Boolean>
        get() = _isSuggestedDocsVisible

    val _usage = MutableLiveData<SpaceUsed?>()
    val usage: LiveData<SpaceUsed?>
        get() = _usage

    val _syncStatus = MutableLiveData<String?>()
    val syncStatus: LiveData<String?>
        get() = _syncStatus

    val _isSyncing = MutableLiveData(false)
    val isSyncing: LiveData<Boolean>
        get() = _isSyncing

    val jsonParser = Json {
        ignoreUnknownKeys = true
    }

    init {
        startUpInRoot()
        getStatus()
    }

    private fun getStatus(){
        val raw = Lb.getStatus()
        val status : LbStatus= jsonParser.decodeFromString(raw)
        hydrateStatusUpdate(status, null)
    }

    private fun checkUsage(){

        if (_usage.value == null || _usage.value?.serverUsage == null){
            return
        }

        val dataCap = _usage.value?.dataCap?.exact?.toFloat() ?: 1f
        val serverUsage = _usage.value?.serverUsage?.exact?.toFloat() ?: 1f

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

        suggestedDocs.set(fileModel.suggestedDocs.intoSuggestedViewHolderInfo(fileModel.idsAndFiles))

        maybeToggleSuggestedDocs()
        files.set(fileModel.children.intoViewHolderInfo(dirtyLocally.value, pullingFiles.value))

        _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpdateBreadcrumbBar)
    }

    /// when you pass in null, it will return the parent of the current folder
    fun enterFolder(newParent: File?) {
        val parent = newParent ?: fileModel.idsAndFiles[fileModel.parent.parent] ?: fileModel.root
        fileModel.enterFolder(parent)

        maybeToggleSuggestedDocs()
        files.set(fileModel.children.intoViewHolderInfo(dirtyLocally.value, pullingFiles.value))

        _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpdateBreadcrumbBar)
    }

    fun reloadFiles() {
        fileModel.refreshFiles()

        files.set(fileModel.children.intoViewHolderInfo(dirtyLocally.value, pullingFiles.value))
        suggestedDocs.set(fileModel.suggestedDocs.intoSuggestedViewHolderInfo(fileModel.idsAndFiles))

        _notifyUpdateFilesUI.value = UpdateFilesUI.ToggleMenuBar
    }


    fun maybeToggleSuggestedDocs() {
        _isSuggestedDocsVisible.value = fileModel.parent.isRoot && !suggestedDocs.isEmpty()
    }

    fun hydrateStatusUpdate(status: LbStatus, lbEvent: LbEvent?){
        if (status.syncStatus!= null){
            _syncStatus.value = status.syncStatus
        }
        _isSyncing.value = status.syncing

        println("syncing status: ${status.syncing}")

        val isMetaDirty = if (lbEvent == null){ true }else{ lbEvent.metadataChanged || lbEvent.pendingSharesChanged }

        if (isMetaDirty){
            _dirtyLocally.value = status.dirtyLocally.mapNotNull { UUID.fromString(it) }.toHashSet()
            _pullingFiles.value = status.pullingFiles.mapNotNull { UUID.fromString(it) }.toHashSet()
            _pushingFiles.value = status.pushingFiles.mapNotNull { UUID.fromString(it) }.toHashSet()
            reloadFiles()

            _usage.value = status.spaceUsed
            checkUsage()
        }
    }
}
