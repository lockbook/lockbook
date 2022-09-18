package app.lockbook.model

import android.app.Application
import android.graphics.BitmapFactory
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.R
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.Json
import java.io.File

class DetailScreenLoaderViewModel(application: Application, val loadingInfo: DetailScreen.Loading) :
    AndroidViewModel(application) {
    private val _updateDetailScreenLoaderUI = SingleMutableLiveData<UpdateDetailScreenLoaderUI>()

    val updateDetailScreenLoaderUI: LiveData<UpdateDetailScreenLoaderUI>
        get() = _updateDetailScreenLoaderUI

    init {
        viewModelScope.launch(Dispatchers.IO) {
            loadContent(loadingInfo)
        }
    }

    private fun loadContent(loadingInfo: DetailScreen.Loading) {
        val extensionHelper = ExtensionHelper(loadingInfo.file.name)

        val updateUI = when {
            extensionHelper.isDrawing -> {
                val json = when (val readDocumentResult = CoreModel.readDocument(loadingInfo.file.id)) {
                    is Ok -> readDocumentResult.value
                    is Err ->
                        return _updateDetailScreenLoaderUI.postValue(
                            UpdateDetailScreenLoaderUI.NotifyError(
                                readDocumentResult.error.toLbError(getRes())
                            )
                        )
                }

                if (json.isEmpty()) {
                    UpdateDetailScreenLoaderUI.NotifyFinished(
                        DetailScreen.Drawing(
                            loadingInfo.file,
                            Drawing()
                        )
                    )
                } else {
                    try {
                        UpdateDetailScreenLoaderUI.NotifyFinished(
                            DetailScreen.Drawing(
                                loadingInfo.file,
                                Json.decodeFromString(json)
                            )
                        )
                    } catch (e: Exception) {
                        UpdateDetailScreenLoaderUI.NotifyError(
                            LbError.newUserError(
                                getString(R.string.drawing_parse_error)
                            )
                        )
                    }
                }
            }
            extensionHelper.isImage -> {
                val bytes = CoreModel.readDocumentBytes(loadingInfo.file.id)

                if (bytes == null) {
                    UpdateDetailScreenLoaderUI.NotifyError(LbError.basicError(getRes()))
                } else {
                    UpdateDetailScreenLoaderUI.NotifyFinished(
                        DetailScreen.ImageViewer(
                            loadingInfo.file,
                            BitmapFactory.decodeByteArray(bytes, 0, bytes.size)
                        )
                    )
                }
            }
            extensionHelper.isPdf -> {
                val child = File(getContext().cacheDir, OPENED_FILE_FOLDER)
                child.deleteRecursively()
                child.mkdir()

                when (val exportFileResult = CoreModel.exportFile(loadingInfo.file.id, child.toString(), true)) {
                    is Ok -> UpdateDetailScreenLoaderUI.NotifyFinished(DetailScreen.PdfViewer(loadingInfo.file, child))
                    is Err -> {
                        UpdateDetailScreenLoaderUI.NotifyError(exportFileResult.error.toLbError(getRes()))
                    }
                }
            }
            else -> {
                val text = when (val readDocumentResult = CoreModel.readDocument(loadingInfo.file.id)) {
                    is Ok -> readDocumentResult.value
                    is Err ->
                        return _updateDetailScreenLoaderUI.postValue(
                            UpdateDetailScreenLoaderUI.NotifyError(
                                readDocumentResult.error.toLbError(getRes())
                            )
                        )
                }

                UpdateDetailScreenLoaderUI.NotifyFinished(
                    DetailScreen.TextEditor(loadingInfo.file, text)
                )
            }
        }

        _updateDetailScreenLoaderUI.postValue(updateUI)
    }
}

sealed class UpdateDetailScreenLoaderUI {
    data class NotifyFinished(val newScreen: DetailScreen) : UpdateDetailScreenLoaderUI()
    data class NotifyError(val error: LbError) : UpdateDetailScreenLoaderUI()
}

class ExtensionHelper(fileName: String) {
    val extension = File(fileName).extension

    val isImage: Boolean
        get() = extension in setOf(
            "jpeg",
            "jpg",
            "png"
        )

    val isDrawing: Boolean get() = extension == "draw"

    val isPdf: Boolean get() = extension == "pdf"
}

const val OPENED_FILE_FOLDER = "opened-files/"
