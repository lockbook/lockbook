package app.lockbook.model

import android.app.Application
import android.os.Handler
import android.os.Looper
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.egui_editor.EditorResponse
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import kotlinx.coroutines.*

class TextEditorViewModel(application: Application, val fileMetadata: File, private val text: String) :
    AndroidViewModel(application) {

    private val handler = Handler(Looper.myLooper()!!)
    var lastEdit = 0L
    var isDirty = false
    var currentContent = text

    private val _updateContent = MutableLiveData<Unit>()
    private val _notifyError = SingleMutableLiveData<LbError>()
    val _editorUpdate = MutableLiveData<EditorResponse>()

    val notifyError: LiveData<LbError>
        get() = _notifyError

    val updateContent: LiveData<Unit>
        get() = _updateContent

    val editorUpdate: LiveData<EditorResponse>
        get() = _editorUpdate

    init {
        setUpTextView()
    }

    private fun setUpTextView() {
        _updateContent.postValue(Unit)
    }

    fun waitAndSaveContents(content: String) {
        currentContent = content
        isDirty = true
        lastEdit = System.currentTimeMillis()
        val currentEdit = lastEdit

        handler.postDelayed(
            {
                viewModelScope.launch(Dispatchers.IO) {
                    if (currentEdit == lastEdit && isDirty) {
                        val writeToDocumentResult =
                            CoreModel.writeToDocument(fileMetadata.id, content)
                        if (writeToDocumentResult is Err) {
                            _notifyError.postValue(writeToDocumentResult.error.toLbError(getRes()))
                        } else {
                            isDirty = false
                        }
                    }
                }
            },
            500
        )
    }
}
