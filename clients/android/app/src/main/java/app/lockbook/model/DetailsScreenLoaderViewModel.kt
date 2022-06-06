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
import timber.log.Timber

class DetailsScreenLoaderViewModel(application: Application, loadingInfo: DetailsScreen.Loading) : AndroidViewModel(application) {
    private val _updateDetailScreenLoaderUI = SingleMutableLiveData<UpdateDetailScreenLoaderUI>()

    val updateDetailScreenLoaderUI: LiveData<UpdateDetailScreenLoaderUI>
        get() = _updateDetailScreenLoaderUI

    init {
        viewModelScope.launch(Dispatchers.IO) {
            loadContent(loadingInfo)
        }
    }

    private fun loadContent(loadingInfo: DetailsScreen.Loading) {
        Timber.e("1 HERE")
        val updateDetailScreenLoaderUI = when(val readDocumentResult = CoreModel.readDocument(loadingInfo.fileMetadata.id)) {
            is Ok -> {
                when {
                    loadingInfo.fileMetadata.decryptedName.endsWith(".draw") -> {
                        Timber.e("2 HERE")

                        val content = String(readDocumentResult.value)
                        Timber.e("3 HERE")
                        if(content.isEmpty()) {
                            UpdateDetailScreenLoaderUI.NotifyFinished(
                                DetailsScreen.Drawing(
                                    loadingInfo.fileMetadata,
                                    Drawing()
                                )
                            )
                        } else {
                            try {
                                val a = UpdateDetailScreenLoaderUI.NotifyFinished(
                                    DetailsScreen.Drawing(
                                        loadingInfo.fileMetadata,
                                        Json.decodeFromString(content)
                                    )
                                )
                                Timber.e("4 HERE")

                                a
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
                                UpdateDetailScreenLoaderUI.NotifyFinished(
                                    DetailsScreen.ImageViewer(
                                        loadingInfo.fileMetadata,
                                        BitmapFactory.decodeByteArray(readDocumentResult.value, 0, readDocumentResult.value.size)
                                    )
                                )
                            }
                    else -> {
                        UpdateDetailScreenLoaderUI.NotifyFinished(
                            DetailsScreen.TextEditor(
                                loadingInfo.fileMetadata,
                                String(readDocumentResult.value)
                            )
                        )
                    }
                }
            }
            is Err -> UpdateDetailScreenLoaderUI.NotifyError(
                readDocumentResult.error.toLbError(
                    getRes()
                )
            )
        }

        _updateDetailScreenLoaderUI.postValue(updateDetailScreenLoaderUI)
    }
}

sealed class UpdateDetailScreenLoaderUI {
    data class NotifyFinished(val newScreen: DetailsScreen) : UpdateDetailScreenLoaderUI()
    data class NotifyError(val error: LbError) : UpdateDetailScreenLoaderUI()
}


