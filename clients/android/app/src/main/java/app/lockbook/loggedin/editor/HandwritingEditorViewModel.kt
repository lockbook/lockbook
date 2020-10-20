package app.lockbook.loggedin.editor

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import app.lockbook.utils.*
import app.lockbook.utils.Messages.UNEXPECTED_ERROR
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.*
import timber.log.Timber

class HandwritingEditorViewModel(
    application: Application,
    private val id: String
) : AndroidViewModel(application) {
    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)
    var lockBookDrawable: Drawing? = null
    private val config = Config(getApplication<Application>().filesDir.absolutePath)
    private val _errorHasOccurred = MutableLiveData<String>()
    private val _unexpectedErrorHasOccurred = MutableLiveData<String>()

    val errorHasOccurred: LiveData<String>
        get() = _errorHasOccurred

    val unexpectedErrorHasOccurred: LiveData<String>
        get() = _unexpectedErrorHasOccurred

    fun handleReadDocument(id: String): String? {
        when (val documentResult = CoreModel.getDocumentContent(config, id)) {
            is Ok -> {
                return documentResult.value.secret
            }
            is Err -> when (val error = documentResult.error) {
                is ReadDocumentError.TreatedFolderAsDocument -> _errorHasOccurred.postValue("Error! Folder treated as document!")
                is ReadDocumentError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                is ReadDocumentError.FileDoesNotExist -> _errorHasOccurred.postValue("Error! File does not exist!")
                is ReadDocumentError.Unexpected -> {
                    Timber.e("Unable to get content of file: ${error.error}")
                    _unexpectedErrorHasOccurred.postValue(
                        UNEXPECTED_ERROR
                    )
                }
            }
        }

        return null
    }

    fun savePath(drawing: Drawing) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                val writeToDocumentResult = CoreModel.writeContentToDocument(config, id, Klaxon().toJsonString(drawing))

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
                        is WriteToDocumentError.Unexpected -> {
                            Timber.e("Unable to write document changes: ${error.error}")
                            _unexpectedErrorHasOccurred.postValue(
                                UNEXPECTED_ERROR
                            )
                        }
                    }
                }
            }
        }
    }
}
