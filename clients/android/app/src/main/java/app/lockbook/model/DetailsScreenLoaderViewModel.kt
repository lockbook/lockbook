package app.lockbook.model

import android.app.Application
import android.graphics.BitmapFactory
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.R
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

class DetailsScreenLoaderViewModel(application: Application, loadingInfo: DetailsScreen.Loading) :
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
        val updateUI = when {
            loadingInfo.fileMetadata.decryptedName.endsWith(".draw") -> {
                val json = when(val readDocumentResult = CoreModel.readDocument(loadingInfo.fileMetadata.id)) {
                    is Ok -> readDocumentResult.value
                    is Err -> return _updateDetailScreenLoaderUI.postValue(UpdateDetailScreenLoaderUI.NotifyError(
                        readDocumentResult.error.toLbError(
                            getRes()
                        ))
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
            loadingInfo.fileMetadata.decryptedName.endsWith(".jpeg")
                    || loadingInfo.fileMetadata.decryptedName.endsWith(".png")
                    || loadingInfo.fileMetadata.decryptedName.endsWith(".jpg") -> {
                val bytes = when(val readDocumentResult = CoreModel.readDocumentBytes(loadingInfo.fileMetadata.id)) {
                    is Ok -> readDocumentResult.value
                    is Err -> return _updateDetailScreenLoaderUI.postValue(UpdateDetailScreenLoaderUI.NotifyError(
                        readDocumentResult.error.toLbError(
                            getRes()
                        ))
                    )
                }

                UpdateDetailScreenLoaderUI.NotifyFinished(
                    DetailsScreen.ImageViewer(
                        loadingInfo.fileMetadata,
                        BitmapFactory.decodeByteArray(bytes, 0, bytes.size)
                    )
                )
            }
            else -> {
                val text = when(val readDocumentResult = CoreModel.readDocument(loadingInfo.fileMetadata.id)) {
                    is Ok -> readDocumentResult.value
                    is Err -> return _updateDetailScreenLoaderUI.postValue(UpdateDetailScreenLoaderUI.NotifyError(
                        readDocumentResult.error.toLbError(
                            getRes()
                        ))
                    )
                }

                UpdateDetailScreenLoaderUI.NotifyFinished(
                    DetailsScreen.TextEditor(
                        loadingInfo.fileMetadata,
                        text
                    )
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


