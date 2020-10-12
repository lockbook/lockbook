package app.lockbook.loggedin.texteditor

import android.text.Editable
import android.text.TextWatcher
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import app.lockbook.utils.Config
import app.lockbook.utils.CoreModel
import app.lockbook.utils.Messages.UNEXPECTED_ERROR
import app.lockbook.utils.WriteToDocumentError
import com.github.michaelbull.result.Err
import timber.log.Timber

class TextEditorViewModel(private val id: String, path: String, initialContents: String) :
    ViewModel(), TextWatcher {
    private val config = Config(path)
    private var history = mutableListOf<String>()
    private var historyIndex = 0
    var ignoreChange = false
    private val _canUndo = MutableLiveData<Boolean>()
    private val _canRedo = MutableLiveData<Boolean>()
    private val _errorHasOccurred = MutableLiveData<String>()

    val canUndo: LiveData<Boolean>
        get() = _canUndo

    val canRedo: LiveData<Boolean>
        get() = _canRedo

    val errorHasOccurred: LiveData<String>
        get() = _errorHasOccurred

    init {
        history.add(history.lastIndex + 1, initialContents)
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

    fun writeNewTextToDocument(content: String) {
        val writeToDocumentResult = CoreModel.writeContentToDocument(config, id, content)
        if (writeToDocumentResult is Err) {
            when (val error = writeToDocumentResult.error) {
                is WriteToDocumentError.FolderTreatedAsDocument -> {
                    _errorHasOccurred.postValue("Error! Folder is treated as document!")
                }
                is WriteToDocumentError.FileDoesNotExist -> {
                    _errorHasOccurred.postValue("Error! File does not exist!")
                }
                is WriteToDocumentError.NoAccount -> {
                    _errorHasOccurred.postValue("Error! No account!")
                }
                is WriteToDocumentError.UnexpectedError -> {
                    Timber.e("Unable to write document changes: ${error.error}")
                    _errorHasOccurred.postValue(
                        UNEXPECTED_ERROR
                    )
                }
                else -> {
                    Timber.e("WriteToDocumentError not matched: ${error::class.simpleName}.")
                    _errorHasOccurred.postValue(UNEXPECTED_ERROR)
                }
            }
        }
    }
}
