package app.lockbook.model

import android.app.Application
import android.graphics.BitmapFactory
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.R
import app.lockbook.getContext
import app.lockbook.getRes
import app.lockbook.getString
import app.lockbook.util.Drawing
import app.lockbook.util.LbError
import app.lockbook.util.SingleMutableLiveData
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.Json
import java.io.File

class DetailsScreenLoaderViewModel(application: Application, val loadingInfo: DetailsScreen.Loading) :
    AndroidViewModel(application) {
    private val _updateDetailScreenLoaderUI = SingleMutableLiveData<UpdateDetailScreenLoaderUI>()

    val updateDetailScreenLoaderUI: LiveData<UpdateDetailScreenLoaderUI>
        get() = _updateDetailScreenLoaderUI

    init {
        viewModelScope.launch(Dispatchers.IO) {
            loadContent(loadingInfo)
        }
    }

    private fun loadContent(loadingInfo: DetailsScreen.Loading) {
        val extensionHelper = ExtensionHelper(loadingInfo.fileMetadata.decryptedName)

        val updateUI = when {
            extensionHelper.isDrawing -> {
                val json = when (val readDocumentResult = CoreModel.readDocument(loadingInfo.fileMetadata.id)) {
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
                        DetailsScreen.Drawing(
                            loadingInfo.fileMetadata,
                            Drawing()
                        )
                    )
                } else {
                    try {
                        UpdateDetailScreenLoaderUI.NotifyFinished(
                            DetailsScreen.Drawing(
                                loadingInfo.fileMetadata,
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
                val bytes = CoreModel.readDocumentBytes(loadingInfo.fileMetadata.id)

                if (bytes == null) {
                    UpdateDetailScreenLoaderUI.NotifyError(LbError.basicError(getRes()))
                } else {
                    UpdateDetailScreenLoaderUI.NotifyFinished(
                        DetailsScreen.ImageViewer(
                            loadingInfo.fileMetadata,
                            BitmapFactory.decodeByteArray(bytes, 0, bytes.size)
                        )
                    )
                }
            }
            extensionHelper.isPdf -> {
                val child = File(getContext().cacheDir, OPENED_FILE_FOLDER)
                child.deleteRecursively()
                child.mkdir()

                when (val exportFileResult = CoreModel.exportFile(loadingInfo.fileMetadata.id, child.toString(), true)) {
                    is Ok -> UpdateDetailScreenLoaderUI.NotifyFinished(DetailsScreen.PdfViewer(loadingInfo.fileMetadata, child))
                    is Err -> {
                        UpdateDetailScreenLoaderUI.NotifyError(exportFileResult.error.toLbError(getRes()))
                    }
                }
            }
            else -> {
                val text = when (val readDocumentResult = CoreModel.readDocument(loadingInfo.fileMetadata.id)) {
                    is Ok -> readDocumentResult.value
                    is Err ->
                        return _updateDetailScreenLoaderUI.postValue(
                            UpdateDetailScreenLoaderUI.NotifyError(
                                readDocumentResult.error.toLbError(getRes())
                            )
                        )
                }

                UpdateDetailScreenLoaderUI.NotifyFinished(
                    DetailsScreen.TextEditor(loadingInfo.fileMetadata, text)
                )
            }
        }

        _updateDetailScreenLoaderUI.postValue(updateUI)
    }
}

sealed class UpdateDetailScreenLoaderUI {
    data class NotifyFinished(val newScreen: DetailsScreen) : UpdateDetailScreenLoaderUI()
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
