package app.lockbook.model

import android.app.Application
import android.view.View
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.App.Companion.config
import app.lockbook.ui.DrawingView.Tool
import app.lockbook.util.*
import app.lockbook.util.ColorAlias
import app.lockbook.util.Drawing
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.*

class DrawingViewModel(
    application: Application,
    private val id: String
) : AndroidViewModel(application) {
    var backupDrawing: Drawing? = null

    private var selectedTool: Tool = Tool.Pen(ColorAlias.White)

    private val _setToolsVisibility = MutableLiveData<Int>()
    private val _selectNewTool = MutableLiveData<Pair<Tool?, Tool>>()
    private val _selectedNewPenSize = MutableLiveData<Int>()
    private val _drawableReady = SingleMutableLiveData<Unit>()
    private val _notifyError = MutableLiveData<LbError>()

    val setToolsVisibility: LiveData<Int>
        get() = _setToolsVisibility

    val selectNewTool: LiveData<Pair<Tool?, Tool>>
        get() = _selectNewTool

    val selectedNewPenSize: LiveData<Int>
        get() = _selectedNewPenSize

    val notifyError: LiveData<LbError>
        get() = _notifyError

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
        when (val documentResult = CoreModel.readDocument(config, id)) {
            is Ok -> {
                return documentResult.value
            }
            is Err -> _notifyError.postValue(documentResult.error.toLbError(getRes()))
        }.exhaustive

        return null
    }

    fun saveDrawing(drawing: Drawing) {
        viewModelScope.launch(Dispatchers.IO) {
            val writeToDocumentResult = CoreModel.writeToDocument(config, id, Klaxon().toJsonString(drawing).replace(" ", ""))

            if (writeToDocumentResult is Err) {
                _notifyError.postValue(writeToDocumentResult.error.toLbError(getRes()))
            }
        }
    }

    fun handleNewToolSelected(newTool: Tool) {
        _selectNewTool.postValue(Pair(selectedTool, newTool))
        selectedTool = newTool
    }

    fun handleNewPenSizeSelected(newPenSize: Int) {
        _selectedNewPenSize.postValue(newPenSize)
    }
}
