package app.lockbook.model

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.App
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

class DetailsScreenLoaderViewModel(application: Application, loadingInfo: DetailsScreen.Loading) : AndroidViewModel(application) {
    private val _updateDetailScreenLoaderUI = SingleMutableLiveData<UpdateDetailScreenLoaderUI>()

    val updateDetailScreenLoaderUI: LiveData<UpdateDetailScreenLoaderUI>
        get() = _updateDetailScreenLoaderUI

    init {
        viewModelScope.launch(Dispatchers.IO) {
            val getContentsResults = CoreModel.readDocument(App.config, loadingInfo.fileMetadata.id)
            _updateDetailScreenLoaderUI.postValue(
                when (getContentsResults) {
                    is Ok -> {
                        when (loadingInfo.fileMetadata.decryptedName.endsWith(".draw")) {
                            true -> {
                                if (getContentsResults.value.isEmpty()) {
                                    UpdateDetailScreenLoaderUI.NotifyFinished(DetailsScreen.Drawing(loadingInfo.fileMetadata, Drawing()))
                                } else {
                                    try {
                                        UpdateDetailScreenLoaderUI.NotifyFinished(DetailsScreen.Drawing(loadingInfo.fileMetadata, Json.decodeFromString(getContentsResults.value)))
                                    } catch (e: Exception) {
                                        UpdateDetailScreenLoaderUI.NotifyError(LbError.newUserError(getString(R.string.drawing_parse_error)))
                                    }
                                }
                            }
                            false -> UpdateDetailScreenLoaderUI.NotifyFinished(DetailsScreen.TextEditor(loadingInfo.fileMetadata, getContentsResults.value))
                        }
                    }
                    is Err -> UpdateDetailScreenLoaderUI.NotifyError(getContentsResults.error.toLbError(getRes()))
                }
            )
        }
    }
}

sealed class UpdateDetailScreenLoaderUI {
    data class NotifyFinished(val newScreen: DetailsScreen) : UpdateDetailScreenLoaderUI()
    data class NotifyError(val error: LbError) : UpdateDetailScreenLoaderUI()
}
