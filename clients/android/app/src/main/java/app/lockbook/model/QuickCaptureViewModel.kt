package app.lockbook.model

import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.lockbook.util.SingleMutableLiveData
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.lockbook.Lb
import net.lockbook.LbError
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

class QuickCaptureViewModel(
    private val savedState: SavedStateHandle,
) : ViewModel() {
    val draft: LiveData<String> = savedState.getLiveData("draft", "")
    private val saving = MutableLiveData(false)
    val isSaving: LiveData<Boolean> = saving
    private val saveError = MutableLiveData<String?>(null)
    val error: LiveData<String?> = saveError
    private val savedNote = SingleMutableLiveData<CapturedNote>()
    val createdNote: LiveData<CapturedNote> = savedNote

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

    fun createStarter(
        template: StarterDocument,
        name: String,
        content: String,
    ) {
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
                        val root = Lb.getRoot()
                        Lb.createFile(Lb.nextName(root.id, name), root.id, true).id
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
