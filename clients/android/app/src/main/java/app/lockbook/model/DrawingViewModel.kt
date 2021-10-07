package app.lockbook.model

import android.app.Application
import android.os.Handler
import android.os.Looper
import androidx.lifecycle.*
import app.lockbook.App.Companion.config
import app.lockbook.getRes
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
    private val handler = Handler(Looper.myLooper()!!)

    var persistentDrawing: Drawing? = null
    var selectedTool: Tool = Tool.Pen(ColorAlias.White)

    private val _drawingReady = SingleMutableLiveData<Drawing>()
    private val _notifyError = SingleMutableLiveData<LbError>()

    val drawingReady: LiveData<Drawing>
        get() = _drawingReady

    val notifyError: LiveData<LbError>
        get() = _notifyError

    fun getDrawing(id: String) {
        viewModelScope.launch(Dispatchers.IO) {
            val contents = when (val documentResult = CoreModel.readDocument(config, id)) {
                is Ok -> {
                    documentResult.value
                }
                is Err -> {
                    _notifyError.postValue(documentResult.error.toLbError(getRes()))
                    return@launch
                }
            }

            val drawing = when {
                persistentDrawing is Drawing -> persistentDrawing!!
                contents.isNotEmpty() -> Klaxon().parse<Drawing>(contents)!!
                else -> Drawing()
            }

            _drawingReady.postValue(drawing)
        }
    }


    fun saveDrawing(drawing: Drawing) {
        viewModelScope.launch(Dispatchers.IO) {
            val writeToDocumentResult = CoreModel.writeToDocument(config, id, Klaxon().toJsonString(drawing).replace(" ", ""))

            if (writeToDocumentResult is Err) {
                _notifyError.postValue(writeToDocumentResult.error.toLbError(getRes()))
            }
        }
    }
}
