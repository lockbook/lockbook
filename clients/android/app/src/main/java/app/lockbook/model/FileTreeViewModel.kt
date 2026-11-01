@file:Suppress(
    "ktlint:standard:backing-property-naming",
    "ktlint:standard:no-wildcard-imports",
    "ktlint:standard:property-naming",
)

package app.lockbook.model

import android.app.Application
import android.content.Context
import androidx.core.content.edit
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.emptyPreferences
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import androidx.preference.PreferenceManager
import app.lockbook.R
import app.lockbook.screen.UpdateFilesUI
import app.lockbook.ui.BreadCrumbItem
import app.lockbook.util.*
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.lockbook.File
import net.lockbook.Lb
import net.lockbook.LbError
import net.lockbook.LbStatus
import net.lockbook.SharingContact
import net.lockbook.Usage
import org.json.JSONArray
import org.json.JSONObject
import timber.log.Timber
import java.io.IOException
import java.util.UUID

data class PinnedFile(
    val id: String,
    val emoji: String? = null,
)

private const val PINNED_FILES_DATASTORE_NAME = "pinned_files"
private const val PINNED_FILES_KEY = "pinned_files"
private val pinnedFilesPreferenceKey = stringPreferencesKey(PINNED_FILES_KEY)
private val Context.pinnedFilesDataStore by preferencesDataStore(name = PINNED_FILES_DATASTORE_NAME)

internal fun observePinnedFiles(context: Context) =
    context.pinnedFilesDataStore.data
        .catch { exception ->
            if (exception is IOException) {
                Timber.e(exception, "Unable to read pinned files.")
                emit(emptyPreferences())
            } else {
                throw exception
            }
        }.map { preferences ->
            deserializePinnedFiles(preferences[pinnedFilesPreferenceKey] ?: "[]")
        }.flowOn(Dispatchers.IO)

private fun deserializePinnedFiles(serializedPins: String): MutableList<PinnedFile> =
    runCatching {
        JSONArray(serializedPins)
            .let { pins ->
                MutableList(pins.length()) { index ->
                    val pin = pins.getJSONObject(index)
                    PinnedFile(pin.getString("id"), pin.optString("emoji").takeIf { it.isNotBlank() })
                }
            }.distinctBy { it.id }
            .toMutableList()
    }.onFailure { exception ->
        Timber.e(exception, "Unable to parse pinned files.")
    }.getOrDefault(mutableListOf())

class FileTreeViewModel(
    application: Application,
) : AndroidViewModel(application) {
    val _notifyUpdateFilesUI = SingleMutableLiveData<UpdateFilesUI>()
    val notifyUpdateFilesUI: LiveData<UpdateFilesUI>
        get() = _notifyUpdateFilesUI

    var fileModel: FileModel? = null
        private set

    private val _sharingContacts = MutableLiveData<List<SharingContact>>(emptyList())
    val sharingContacts: LiveData<List<SharingContact>> = _sharingContacts

    // / the list of files that the file tree displays in the UI
    val _files = MutableLiveData<List<FileViewHolderInfo>>(emptyList())
    val files: LiveData<List<FileViewHolderInfo>>
        get() = _files

    private val _recentFiles = MutableLiveData<List<File>>(emptyList())
    val recentFiles: LiveData<List<File>>
        get() = _recentFiles

    private var pinnedFileEntries = mutableListOf<PinnedFile>()
    private var persistPinnedFilesJob: Job? = null
    val _pinnedFiles = MutableLiveData<List<PinnedFile>>(emptyList())
    val pinnedFiles: LiveData<List<PinnedFile>>
        get() = _pinnedFiles

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
        if (FileMetadataStore.snapshots.value == null) FileMetadataStore.requestRefresh()
        viewModelScope.launch {
            FileMetadataStore.snapshots.collect { snapshot ->
                val previous = fileModel
                if (snapshot == null) {
                    fileModel = null
                    _files.value = emptyList()
                    _recentFiles.value = emptyList()
                } else if (previous?.idsAndFiles !== snapshot.idsAndFiles) {
                    val previousPath = previous?.getFileDir()?.map { it.id to it.name }
                    fileModel =
                        if (previous == null || previous.root.id != snapshot.root.id) {
                            FileModel.createAtRoot(snapshot)
                        } else {
                            previous.apply { refreshFiles(snapshot) }
                        }
                    refreshFilesDataSource()
                    if (previousPath != fileModel?.getFileDir()?.map { it.id to it.name }) {
                        _notifyUpdateFilesUI.value = UpdateFilesUI.UpdateBreadcrumbBar
                    }
                }
            }
        }
        viewModelScope.launch {
            try {
                val contacts: List<SharingContact> = withContext(Dispatchers.IO) { Lb.getSharingContacts().toList() }
                _sharingContacts.value = contacts
            } catch (error: LbError) {
                Timber.e(error, "Unable to load sharing contacts")
            }
        }
        loadPinnedFiles()
        viewModelScope.launch {
            try {
                val status = withContext(Dispatchers.IO) { Lb.getStatus() }
                hydrateStatusUpdate(status)
            } catch (error: LbError) {
                Timber.e(error, "Unable to load account status.")
            }
        }
    }

    private fun checkUsage() {
        if (usage.value == null || _usage.value?.serverUsage == null) {
            return
        }

        val dataCap =
            usage.value
                ?.dataCap
                ?.exact
                ?.toFloat() ?: 1f
        val serverUsage =
            usage.value
                ?.serverUsage
                ?.exact
                ?.toFloat() ?: 1f

        val usageRatio = serverUsage / dataCap

        val pref = PreferenceManager.getDefaultSharedPreferences(getContext())

        val showOutOfSpace09 = pref.getBoolean(getString(R.string.show_running_out_of_space_0_9_key), true)
        val showOutOfSpace08 = pref.getBoolean(getString(R.string.show_running_out_of_space_0_8_key), true)

        when {
            usageRatio >= 1.0 -> {}

            usageRatio > 0.9 -> {
                if (!showOutOfSpace09) {
                    return
                }
            }

            usageRatio > 0.8 -> {
                if (!showOutOfSpace08) {
                    return
                }
            }

            else -> {
                if (!showOutOfSpace09) {
                    pref.edit {
                        putBoolean(getString(R.string.show_running_out_of_space_0_9_key, true), true)
                    }
                }

                if (!showOutOfSpace08) {
                    pref.edit {
                        putBoolean(getString(R.string.show_running_out_of_space_0_8_key, true), true)
                    }
                }

                return
            }
        }

        _notifyUpdateFilesUI.postValue(UpdateFilesUI.OutOfSpace((usageRatio * 100).toInt(), 100))
    }

    // / when you pass in null, it will return the parent of the current folder
    fun enterFolder(newParent: File?) {
        val navigation = fileModel ?: return
        val parent = newParent ?: navigation.idsAndFiles[navigation.parent.parent] ?: navigation.root
        navigation.enterFolder(parent)

        refreshFilesDataSource()

        _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpdateBreadcrumbBar)
    }

    fun reloadFiles() {
        val snapshot = FileMetadataStore.snapshots.value ?: return
        fileModel?.refreshFiles(snapshot)

        refreshFilesDataSource()
    }

    private fun refreshFilesDataSource() {
        val fileModel = fileModel ?: return
        _files.value = fileModel.children.intoViewHolderInfo(dirtyLocally.value, pullingFiles.value)
        _recentFiles.value = recentDocuments(fileModel.idsAndFiles.values)
        refreshPinnedFiles()
    }

    fun pinFiles(files: List<File>): List<PinnedFile> {
        val newlyPinned =
            files
                .filter { file -> pinnedFileEntries.none { it.id == file.id } }
                .map { file -> PinnedFile(file.id) }

        if (newlyPinned.isNotEmpty()) {
            pinnedFileEntries.addAll(newlyPinned)
            persistPinnedFiles()
            refreshPinnedFiles()
        }

        return newlyPinned
    }

    fun setPinnedEmoji(
        fileId: String,
        emoji: String?,
    ) {
        val index = pinnedFileEntries.indexOfFirst { it.id == fileId }
        if (index == -1) {
            return
        }

        pinnedFileEntries[index] = PinnedFile(fileId, emoji?.takeIf { it.isNotBlank() })
        persistPinnedFiles()
        refreshPinnedFiles()
    }

    fun unpinFile(fileId: String) {
        if (pinnedFileEntries.removeAll { it.id == fileId }) {
            persistPinnedFiles()
            refreshPinnedFiles()
        }
    }

    fun restorePinnedFile(pin: PinnedFile) {
        if (pinnedFileEntries.none { it.id == pin.id }) {
            pinnedFileEntries.add(pin)
            persistPinnedFiles()
            refreshPinnedFiles()
        }
    }

    private fun refreshPinnedFiles() {
        val knownFileIds = fileModel?.idsAndFiles?.keys
        if (knownFileIds == null) {
            _pinnedFiles.value = pinnedFileEntries.toList()
            return
        }
        if (pinnedFileEntries.removeAll { it.id !in knownFileIds }) {
            persistPinnedFiles()
        }

        _pinnedFiles.value = pinnedFileEntries.toList()
    }

    private fun loadPinnedFiles() {
        viewModelScope.launch {
            val pins = observePinnedFiles(getApplication()).first()

            pinnedFileEntries = pins.toMutableList()
            refreshPinnedFiles()
        }
    }

    private fun serializePinnedFiles(pins: List<PinnedFile>): String =
        JSONArray()
            .apply {
                pins.forEach { pin ->
                    put(
                        JSONObject().apply {
                            put("id", pin.id)
                            putOpt("emoji", pin.emoji)
                        },
                    )
                }
            }.toString()

    private fun persistPinnedFiles(pins: List<PinnedFile> = pinnedFileEntries.toList()) {
        persistPinnedFilesJob?.cancel()
        persistPinnedFilesJob =
            viewModelScope.launch(Dispatchers.IO) {
                val serializedPins = serializePinnedFiles(pins)

                try {
                    getApplication<Application>()
                        .pinnedFilesDataStore
                        .edit { preferences ->
                            preferences[pinnedFilesPreferenceKey] = serializedPins
                        }
                } catch (exception: IOException) {
                    Timber.e(exception, "Unable to persist pinned files.")
                }
            }
    }

    fun hydrateStatusUpdate(status: LbStatus) {
        if (status.syncStatus != null) {
            _syncStatus.value = status.syncStatus
        }
        _isSyncing.value = status.syncing

        _dirtyLocally.value =
            status.dirtyLocally
                .orEmpty()
                .mapNotNull { UUID.fromString(it) }
                .toHashSet()
        _pullingFiles.value =
            status.pullingFiles
                .orEmpty()
                .mapNotNull { UUID.fromString(it) }
                .toHashSet()
        _pushingFiles.value =
            status.pushingFiles
                .orEmpty()
                .mapNotNull { UUID.fromString(it) }
                .toHashSet()

        _files.value = fileModel?.children.orEmpty().intoViewHolderInfo(dirtyLocally.value, pullingFiles.value)

        _usage.value = status.spaceUsed
        checkUsage()
    }
}
