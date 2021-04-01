package app.lockbook.model

import android.app.Application
import android.view.View
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.ui.DrawingView
import app.lockbook.util.*
import app.lockbook.util.ColorAlias
import app.lockbook.util.Config
import app.lockbook.util.Drawing
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.*
import timber.log.Timber

class DrawingViewModel(
    application: Application,
    private val id: String
) : AndroidViewModel(application) {
    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)
    private val config = Config(getApplication<Application>().filesDir.absolutePath)
    var backupDrawing: Drawing? = null

    private var selectedTool: DrawingView.Tool = DrawingView.Pen(ColorAlias.White)

    private val _setToolsVisibility = MutableLiveData<Int>()
    private val _selectNewTool = MutableLiveData<Pair<DrawingView.Tool?, DrawingView.Tool>>()
    private val _selectedNewPenSize = MutableLiveData<Int>()
    private val _drawableReady = SingleMutableLiveData<Unit>()
    private val _errorHasOccurred = MutableLiveData<String>()
    private val _unexpectedErrorHasOccurred = MutableLiveData<String>()

    val setToolsVisibility: LiveData<Int>
        get() = _setToolsVisibility

    val selectNewTool: LiveData<Pair<DrawingView.Tool?, DrawingView.Tool>>
        get() = _selectNewTool

    val selectedNewPenSize: LiveData<Int>
        get() = _selectedNewPenSize

    val errorHasOccurred: LiveData<String>
        get() = _errorHasOccurred

    val unexpectedErrorHasOccurred: LiveData<String>
        get() = _unexpectedErrorHasOccurred

    val drawableReady: LiveData<Unit>
        get() = _drawableReady

    init {
        _selectNewTool.postValue(Pair(null, selectedTool))
        _selectedNewPenSize.postValue(7)
    }

    fun handleTouchEvent(toolsVisibility: Int) {
        if (toolsVisibility == View.VISIBLE) {
            _setToolsVisibility.postValue(View.GONE)
        } else {
            _setToolsVisibility.postValue(View.VISIBLE)
        }
    }

    fun getDrawing(id: String) {
        viewModelScope.launch(Dispatchers.IO) {
            val contents = readDocument(id)
            if (contents != null && contents.isEmpty()) {
                backupDrawing = Drawing()
            } else if (contents != null) {
                backupDrawing = Klaxon().parse<Drawing>(contents)
            }

            _drawableReady.postValue(Unit)
        }
    }

    private fun readDocument(id: String): String? {
        when (val documentResult = CoreModel.getDocumentContent(config, id)) {
            is Ok -> {
                return documentResult.value
            }
            is Err -> when (val error = documentResult.error) {
                is ReadDocumentError.TreatedFolderAsDocument -> _errorHasOccurred.postValue("Error! Folder treated as document!")
                is ReadDocumentError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                is ReadDocumentError.FileDoesNotExist -> _errorHasOccurred.postValue("Error! File does not exist!")
                is ReadDocumentError.Unexpected -> {
                    Timber.e("Unable to get content of file: ${error.error}")
                    _unexpectedErrorHasOccurred.postValue(
                        error.error
                    )
                }
            }
        }.exhaustive

        return null
    }

    fun saveDrawing(drawing: Drawing) {
        viewModelScope.launch(Dispatchers.IO) {
            val writeToDocumentResult = CoreModel.writeContentToDocument(config, id, Klaxon().toJsonString(drawing).replace(" ", ""))

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
                            error.error
                        )
                    }
                }.exhaustive
            }
        }
    }

    fun handleNewToolSelected(newTool: DrawingView.Tool) {
        _selectNewTool.postValue(Pair(selectedTool, newTool))
        selectedTool = newTool
    }

    fun handleNewPenSizeSelected(newPenSize: Int) {
        _selectedNewPenSize.postValue(newPenSize)
    }
}
