package app.lockbook.model

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.App
import app.lockbook.getRes
import app.lockbook.util.Drawing
import app.lockbook.util.LbError
import app.lockbook.util.SingleMutableLiveData
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

class DetailsScreenLoaderViewModel(application: Application, loadingInfo: DetailsScreen.Loading) : AndroidViewModel(application) {
    private val _updateDetailScreenLoaderUI = SingleMutableLiveData<UpdateDetailScreenLoaderUI>()

    val updateDetailScreenLoaderUI: LiveData<UpdateDetailScreenLoaderUI>
        get() = _updateDetailScreenLoaderUI

    init {
        viewModelScope.launch(Dispatchers.IO) {
            val getContentsResults = CoreModel.readDocument(App.config, loadingInfo.fileMetadata.id)
            updateDetailScreenLoaderUI(
                when (getContentsResults) {
                    is Ok -> {
                        when (loadingInfo.fileMetadata.name.endsWith(".draw")) {
                            true -> {
                                val drawing = if (getContentsResults.value.isEmpty()) {
                                    Drawing()
                                } else {
                                    Klaxon().parse<Drawing>(getContentsResults.value)!!
                                }

                                UpdateDetailScreenLoaderUI.NotifyFinished(DetailsScreen.Drawing(loadingInfo.fileMetadata, drawing))
                            }
                            false -> UpdateDetailScreenLoaderUI.NotifyFinished(DetailsScreen.TextEditor(loadingInfo.fileMetadata, getContentsResults.value))
                        }
                    }
                    is Err -> UpdateDetailScreenLoaderUI.NotifyError(getContentsResults.error.toLbError(getRes()))
                }
            )
        }
    }

    private fun updateDetailScreenLoaderUI(update: UpdateDetailScreenLoaderUI) {
        _updateDetailScreenLoaderUI.postValue(update)
    }
}

sealed class UpdateDetailScreenLoaderUI {
    data class NotifyFinished(val newScreen: DetailsScreen) : UpdateDetailScreenLoaderUI()
    data class NotifyError(val error: LbError) : UpdateDetailScreenLoaderUI()
}
