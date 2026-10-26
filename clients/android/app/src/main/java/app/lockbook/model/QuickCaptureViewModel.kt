package app.lockbook.model

import android.app.Application
import android.content.Context
import androidx.core.content.edit
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.viewModelScope
import app.lockbook.util.SingleMutableLiveData
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.lockbook.File
import net.lockbook.Lb
import net.lockbook.LbError
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

class QuickCaptureViewModel(
    application: Application,
    private val savedState: SavedStateHandle,
) : AndroidViewModel(application) {
    private val preferences = application.getSharedPreferences("quick_capture", Context.MODE_PRIVATE)
    private val folder = MutableLiveData<File?>()
    val saveFolder: LiveData<File?> = folder

    val draft: LiveData<String> = savedState.getLiveData("draft", "")
    private val saving = MutableLiveData(false)
    val isSaving: LiveData<Boolean> = saving
    private val saveError = MutableLiveData<String?>(null)
    val error: LiveData<String?> = saveError
    private val savedNote = SingleMutableLiveData<CapturedNote>()
    val createdNote: LiveData<CapturedNote> = savedNote

    private var folderRefresh: Job? = null

    init {
        refreshSaveFolder()
    }

    private fun resolveSaveFolder(): File {
        val root = Lb.getRoot()
        val id = preferences.getString("${root.id}.folder", null)
        return id
            ?.let { runCatching { Lb.getFileById(it) }.getOrNull() }
            ?.takeIf { it.type == File.FileType.Folder } ?: root
    }

    fun refreshSaveFolder() {
        folderRefresh?.cancel()
        folderRefresh =
            viewModelScope.launch {
                val selected = withContext(Dispatchers.IO) { runCatching { resolveSaveFolder() }.getOrNull() }
                if (selected != null) folder.value = selected
            }
    }

    fun selectSaveFolder(id: String) {
        if (saving.value == true) return
        folderRefresh?.cancel()
        viewModelScope.launch {
            try {
                folder.value =
                    withContext(Dispatchers.IO) {
                        val selected = Lb.getFileById(id)
                        require(selected.type == File.FileType.Folder)
                        val root = Lb.getRoot()
                        preferences.edit { putString("${root.id}.folder", selected.id) }
                        selected
                    }
            } catch (error: LbError) {
                saveError.value = error.msg
            }
        }
    }

    fun setDraft(text: String) {
        savedState["draft"] = text
        saveError.value = null
    }

    fun save() {
        val content = draft.value.orEmpty()
        if (content.isBlank()) return
        val date = SimpleDateFormat("yyyy-MM-dd", Locale.US).format(Date())
        saveNote("$date.md", content, "pendingNote", clearDraft = true)
    }

    fun createStarter(template: StarterDocument) {
        val context = getApplication<Application>()
        val name = context.getString(template.fileName)
        val content = context.getString(template.content)
        val date = SimpleDateFormat("yyyy-MM-dd", Locale.US).format(Date())
        val fileName = if (template == StarterDocument.Journal) "$date.md" else name
        saveNote(fileName, content, "pendingStarter${template.name}", clearDraft = false)
    }

    private fun saveNote(
        name: String,
        content: String,
        pendingKey: String,
        clearDraft: Boolean,
    ) {
        if (saving.value == true) return
        saving.value = true
        saveError.value = null
        viewModelScope.launch {
            try {
                // Reuse a created file on retry if writing failed, rather than creating duplicates.
                val id =
                    savedState.get<String>(pendingKey) ?: withContext(Dispatchers.IO) {
                        val parent = resolveSaveFolder()
                        folder.postValue(parent)
                        Lb.createFile(Lb.nextName(parent.id, name), parent.id, true).id
                    }.also { savedState[pendingKey] = it }
                var written = content
                while (true) {
                    withContext(Dispatchers.IO) { Lb.writeDocument(id, written) }
                    val latest = if (clearDraft) draft.value.orEmpty() else written
                    if (latest == written) break
                    written = latest
                }
                savedState[pendingKey] = null
                if (clearDraft) savedState["draft"] = ""
                savedNote.value = CapturedNote(id, clearDraft)
            } catch (error: LbError) {
                saveError.value = error.msg
            } finally {
                saving.value = false
            }
        }
    }
}

data class CapturedNote(
    val id: String,
    val fromQuickCapture: Boolean,
)
