package app.lockbook.model

import android.app.Application
import android.text.Editable
import android.text.TextWatcher
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.App.Companion.config
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.*

class TextEditorViewModel(application: Application, private val id: String) :
    AndroidViewModel(application), TextWatcher {
    private var history = mutableListOf<String>()
    private var historyIndex = 0
    var ignoreChange = false
    private val _canUndo = MutableLiveData<Boolean>()
    private val _canRedo = MutableLiveData<Boolean>()
    private val _notifyError = MutableLiveData<LbError>()

    val canUndo: LiveData<Boolean>
        get() = _canUndo

    val canRedo: LiveData<Boolean>
        get() = _canRedo

    val notifyError: LiveData<LbError>
        get() = _notifyError

    init {
        val contents = readDocument(id)
        if (contents != null) {
            history.add(history.lastIndex + 1, contents)
        }
    }

    fun readDocument(id: String): String? {
        when (val documentResult = CoreModel.readDocument(config, id)) {
            is Ok -> {
                return documentResult.value
            }
            is Err -> _notifyError.postValue(documentResult.error.toLbError(getRes()))
        }.exhaustive

        return null
    }

    override fun afterTextChanged(s: Editable?) {
        if (!ignoreChange) {
            if (history.size - 1 > historyIndex) {
                history.subList(historyIndex, history.size).clear()
            }

            if (history.size >= 10) {
                history.removeAt(history.lastIndex)
            } else {
                historyIndex++
            }
            history.add(history.lastIndex + 1, s.toString())
        } else {
            ignoreChange = false
        }
        canUndo()
        canRedo()
    }

    fun undo(): String {
        historyIndex--
        canUndo()
        return history[historyIndex]
    }

    fun redo(): String {
        historyIndex++
        canRedo()
        return history[historyIndex]
    }

    private fun canUndo() {
        _canUndo.value = historyIndex != 0
    }

    private fun canRedo() {
        _canRedo.value = historyIndex != history.lastIndex
    }

    override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}

    override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {}

    fun saveText(content: String) {
        viewModelScope.launch(Dispatchers.IO) {
            val writeToDocumentResult = CoreModel.writeToDocument(config, id, content)
            if (writeToDocumentResult is Err) {
                _notifyError.postValue(writeToDocumentResult.error.toLbError(getRes()))
            }
        }
    }
}
