package app.lockbook.model

import android.app.Application
import android.view.MotionEvent
import android.view.View
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.*
import timber.log.Timber
import java.util.*

class HandwritingEditorViewModel(
    application: Application,
    private val id: String
) : AndroidViewModel(application) {
    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)
    private val config = Config(getApplication<Application>().filesDir.absolutePath)
    private var singleTapTimer = Timer()
    private var areToolsVisible = true
    private var isAnimatingTools = false
    var lockBookDrawable: Drawing? = null

    private val _setToolsVisibility = MutableLiveData<Int>()
    private val _drawableReady = SingleMutableLiveData<Unit>()
    private val _errorHasOccurred = MutableLiveData<String>()
    private val _unexpectedErrorHasOccurred = MutableLiveData<String>()

    val setToolsVisibility: LiveData<Int>
        get() = _setToolsVisibility

    val errorHasOccurred: LiveData<String>
        get() = _errorHasOccurred

    val unexpectedErrorHasOccurred: LiveData<String>
        get() = _unexpectedErrorHasOccurred

    val drawableReady: LiveData<Unit>
        get() = _drawableReady

    fun handleTouchEvent(event: MotionEvent, toolsVisibility: Int) {
        val isCorrectMotionEvent = event.action == MotionEvent.ACTION_DOWN && event.getToolType(0) == MotionEvent.TOOL_TYPE_FINGER

        if (isCorrectMotionEvent && !isAnimatingTools) {
            isAnimatingTools = true

            singleTapTimer.schedule(
                object : TimerTask() {
                    override fun run() {
                        if (toolsVisibility == View.VISIBLE) {
                            _setToolsVisibility.postValue(View.GONE)
                        } else {
                            _setToolsVisibility.postValue(View.VISIBLE)
                        }
                        isAnimatingTools = false
                    }
                },
                200
            )
        }
    }

    fun detectedScale() {
        singleTapTimer.cancel()
        singleTapTimer = Timer()
        isAnimatingTools = false
    }

    fun getDrawing(id: String) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                val contents = readDocument(id)
                if (contents != null && contents.isEmpty()) {
                    lockBookDrawable = Drawing()
                } else if (contents != null) {
                    lockBookDrawable = Klaxon().parse<Drawing>(contents)
                }

                _drawableReady.postValue(Unit)
            }
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

    fun savePath(drawing: Drawing) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
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
    }
}
