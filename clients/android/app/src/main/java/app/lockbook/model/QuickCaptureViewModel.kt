package app.lockbook.model

import android.app.Application
import android.content.Context
import android.content.SharedPreferences
import androidx.core.content.edit
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.viewModelScope
import app.lockbook.util.SingleMutableLiveData
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.lockbook.File
import net.lockbook.Lb
import net.lockbook.LbError

class QuickCaptureViewModel(
    application: Application,
    private val savedState: SavedStateHandle,
) : AndroidViewModel(application) {
    private val preferences = application.getSharedPreferences("quick_capture", Context.MODE_PRIVATE)
    private val folder = MutableLiveData<File?>()
    val saveFolder: LiveData<File?> = folder

    val draft: LiveData<String> = savedState.getLiveData("draft", "")
    var isSaving = false
        private set
    private val saveError = SingleMutableLiveData<String>()
    val error: LiveData<String> = saveError
    private val savedNote = SingleMutableLiveData<String>()
    val createdNote: LiveData<String> = savedNote
    private val preparedNote = MutableLiveData<String?>()
    val readyNote: LiveData<String?> = preparedNote
    private var handoffRequested = false
    val hasPendingCapture: Boolean
        get() = isSaving || savedState.get<String>("pendingNote") != null

    private val preferenceListener =
        SharedPreferences.OnSharedPreferenceChangeListener { _, _ ->
            updateSaveFolder()
        }

    init {
        preferences.registerOnSharedPreferenceChangeListener(preferenceListener)
        viewModelScope.launch {
            FileMetadataStore.snapshots.collect { updateSaveFolder() }
        }
        savedState.get<String>("openingNote")?.let { savedNote.value = it } ?: persistDraft()
    }

    private fun resolveSaveFolder(snapshot: FileMetadataSnapshot): File {
        val id = preferences.getString("${snapshot.root.id}.folder", null)
        return snapshot.idsAndFiles[id]?.takeIf { it.type == File.FileType.Folder } ?: snapshot.root
    }

    private fun updateSaveFolder() {
        folder.value = FileMetadataStore.snapshots.value?.let(::resolveSaveFolder)
    }

    fun selectSaveFolder(id: String) {
        if (hasPendingCapture) return
        viewModelScope.launch {
            try {
                val snapshot = FileMetadataStore.current()
                val selected = FileMetadataStore.requireFile(id)
                if (selected.type != File.FileType.Folder) return@launch
                withContext(Dispatchers.IO) {
                    preferences.edit { putString("${snapshot.root.id}.folder", selected.id) }
                }
            } catch (error: LbError) {
                saveError.value = error.msg
            }
        }
    }

    override fun onCleared() {
        preferences.unregisterOnSharedPreferenceChangeListener(preferenceListener)
        super.onCleared()
    }

    fun setDraft(text: String) {
        savedState["draft"] = text
        persistDraft()
    }

    fun save() {
        handoffRequested = true
        persistDraft()
    }

    fun finishHandoff() {
        preparedNote.value = null
        savedState["draft"] = ""
        savedState["pendingNote"] = null
        savedState["openingNote"] = null
    }

    private fun persistDraft() {
        // Text entered while the native editor loads is transferred at handoff instead of
        // starting another capture or overwriting the document already being opened.
        if (savedState.get<String>("openingNote") != null) return
        val content = draft.value.orEmpty()
        if (content.isBlank() && savedState.get<String>("pendingNote") == null) return
        if (isSaving) return
        isSaving = true
        viewModelScope.launch {
            try {
                // Reuse a created file on retry if writing failed, rather than creating duplicates.
                val id =
                    savedState.get<String>("pendingNote") ?: withContext(Dispatchers.IO) {
                        val parent = resolveSaveFolder(FileMetadataStore.awaitLoaded())
                        val type = NewFileType.Note
                        FileCreation.create(parent.id, type, FileCreation.suggestBaseName(parent.id, type)).id
                    }.also { savedState["pendingNote"] = it }
                var written = content
                while (true) {
                    withContext(Dispatchers.IO) { Lb.writeDocument(id, written) }
                    // Core creation and Android's metadata cache are asynchronous. Do not
                    // announce readiness until the workspace can resolve the new document.
                    val metadata = FileMetadataStore.snapshots.value
                    if (metadata == null || id !in metadata.idsAndFiles) {
                        withContext(Dispatchers.IO) { FileMetadataStore.refreshAndAwait() }
                        FileMetadataStore.requireFile(id)
                    }
                    val latest = draft.value.orEmpty()
                    if (latest == written) break
                    written = latest
                }
                if (handoffRequested && written.isNotBlank()) {
                    handoffRequested = false
                    savedState["openingNote"] = id
                    preparedNote.value = null
                    savedState["draft"] = ""
                    savedNote.value = id
                } else {
                    handoffRequested = false
                    preparedNote.value = id.takeIf { written.isNotBlank() }
                }
            } catch (error: LbError) {
                handoffRequested = false
                preparedNote.value = null
                saveError.value = error.msg
            } finally {
                isSaving = false
            }
        }
    }
}
