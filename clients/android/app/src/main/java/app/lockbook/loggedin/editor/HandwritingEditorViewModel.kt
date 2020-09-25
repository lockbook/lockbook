package app.lockbook.loggedin.editor

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import timber.log.Timber

class HandwritingEditorViewModel(
    application: Application,
    private val id: String
) : AndroidViewModel(application) {
    private val config = Config(getApplication<Application>().filesDir.absolutePath)
    private val _errorHasOccurred = MutableLiveData<String>()

    val errorHasOccurred: LiveData<String>
        get() = _errorHasOccurred

    fun handleReadDocument(id: String): String? {
        when (val documentResult = CoreModel.getDocumentContent(config, id)) {
            is Ok -> {
                return documentResult.value.secret
            }
            is Err -> when (val error = documentResult.error) {
                is ReadDocumentError.TreatedFolderAsDocument -> _errorHasOccurred.postValue("Error! Folder treated as document!")
                is ReadDocumentError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                is ReadDocumentError.FileDoesNotExist -> _errorHasOccurred.postValue("Error! File does not exist!")
                is ReadDocumentError.UnexpectedError -> {
                    Timber.e("Unable to get content of file: ${error.error}")
                    _errorHasOccurred.postValue(
                        Messages.UNEXPECTED_ERROR_OCCURRED
                    )
                }
                else -> {
                    Timber.e("ReadDocumentError not matched: ${error::class.simpleName}.")
                    _errorHasOccurred.postValue(Messages.UNEXPECTED_ERROR_OCCURRED)
                }
            }
        }

        return null
    }


    fun savePath(drawable: Drawing) {
        val writeToDocumentResult = CoreModel.writeContentToDocument(config, id, Klaxon().toJsonString(drawable))

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
                        Messages.UNEXPECTED_ERROR_OCCURRED
                    )
                }
                else -> {
                    Timber.e("WriteToDocumentError not matched: ${error::class.simpleName}.")
                    _errorHasOccurred.postValue(Messages.UNEXPECTED_ERROR_OCCURRED)
                }
            }
        }
    }
}
