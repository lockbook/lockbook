package app.lockbook.model

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.getRes
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import java.io.File

class StateViewModel(application: Application) : AndroidViewModel(application) {
    var detailsScreen: DetailsScreen? = null
    var transientScreen: TransientScreen? = null

    private val _launchDetailsScreen = SingleMutableLiveData<DetailsScreen?>()
    private val _launchTransientScreen = SingleMutableLiveData<TransientScreen>()
    private val _updateMainScreenUI = SingleMutableLiveData<UpdateMainScreenUI>()

    val launchDetailsScreen: LiveData<DetailsScreen?>
        get() = _launchDetailsScreen

    val launchTransientScreen: LiveData<TransientScreen>
        get() = _launchTransientScreen

    val updateMainScreenUI: LiveData<UpdateMainScreenUI>
        get() = _updateMainScreenUI

    val shareModel = ShareModel(_updateMainScreenUI)

    fun launchTransientScreen(screen: TransientScreen) {
        transientScreen = screen
        _launchTransientScreen.postValue(transientScreen)
    }

    fun launchDetailsScreen(screen: DetailsScreen?) {
        detailsScreen = screen
        _launchDetailsScreen.postValue(detailsScreen)
    }

    fun shareSelectedFiles(selectedFiles: List<DecryptedFileMetadata>, appDataDir: File) {
        viewModelScope.launch(Dispatchers.IO) {
            val shareResult = shareModel.shareDocuments(selectedFiles, appDataDir)
            if (shareResult is Err) {
                _updateMainScreenUI.postValue(
                    UpdateMainScreenUI.NotifyError(
                        shareResult.error.toLbError(
                            getRes()
                        )
                    )
                )
                return@launch
            }
        }
    }

    // You can save on exit here since this scope will exist after the editors don't, thus long saves won't be problematic
    fun saveDrawingOnExit(id: String, drawing: Drawing) {
        viewModelScope.launch(Dispatchers.IO) {
            val writeToDocumentResult =
                CoreModel.writeToDocument(
                    id,
                    Json.encodeToString(drawing).replace(" ", "")
                )

            if (writeToDocumentResult is Err) {
                _updateMainScreenUI.postValue(
                    UpdateMainScreenUI.NotifyError(
                        writeToDocumentResult.error.toLbError(
                            getRes()
                        )
                    )
                )
            }
        }
    }

    fun saveTextOnExit(id: String, text: String) {
        viewModelScope.launch(Dispatchers.IO) {
            val writeToDocumentResult =
                CoreModel.writeToDocument(id, text)

            if (writeToDocumentResult is Err) {
                _updateMainScreenUI.postValue(
                    UpdateMainScreenUI.NotifyError(
                        writeToDocumentResult.error.toLbError(
                            getRes()
                        )
                    )
                )
            }
        }
    }

    fun confirmSubscription(purchaseToken: String, accountID: String) {
        viewModelScope.launch(Dispatchers.IO) {
            val confirmSubscriptionResult =
                CoreModel.upgradeAccountAndroid(purchaseToken, accountID)

            when (confirmSubscriptionResult) {
                is Ok -> {
                    _updateMainScreenUI.postValue(UpdateMainScreenUI.ShowSubscriptionConfirmed)
                }
                is Err -> {
                    _updateMainScreenUI.postValue(
                        UpdateMainScreenUI.NotifyError(
                            confirmSubscriptionResult.error.toLbError(
                                getRes()
                            )
                        )
                    )
                }
            }
        }
    }
}

sealed class DetailsScreen(open val fileMetadata: DecryptedFileMetadata) {
    data class Loading(override val fileMetadata: DecryptedFileMetadata) : DetailsScreen(fileMetadata)
    data class TextEditor(override val fileMetadata: DecryptedFileMetadata, val text: String) :
        DetailsScreen(fileMetadata)

    data class Drawing(
        override val fileMetadata: DecryptedFileMetadata,
        val drawing: app.lockbook.util.Drawing
    ) : DetailsScreen(fileMetadata)
}

sealed class TransientScreen {
    data class Move(val ids: Array<String>) : TransientScreen()
    data class Rename(val file: DecryptedFileMetadata) : TransientScreen()
    data class Create(val info: CreateFileInfo) : TransientScreen()
    data class Info(val file: DecryptedFileMetadata) : TransientScreen()
    data class Share(val files: List<File>) : TransientScreen()
}

sealed class UpdateMainScreenUI {
    data class ShowHideProgressOverlay(val show: Boolean) : UpdateMainScreenUI()
    data class ShareDocuments(val files: ArrayList<File>) : UpdateMainScreenUI()
    data class NotifyError(val error: LbError) : UpdateMainScreenUI()
    object ShowSubscriptionConfirmed : UpdateMainScreenUI()
}

data class CreateFileInfo(
    val parentId: String,
    val extendedFileType: ExtendedFileType
)

sealed class ExtendedFileType {
    object Text : ExtendedFileType()
    object Drawing : ExtendedFileType()
    object Folder : ExtendedFileType()

    fun toFileType(): FileType = when (this) {
        Drawing, Text -> FileType.Document
        Folder -> FileType.Folder
    }
}

data class MoveFileInfo(
    val ids: Array<String>,
    val names: Array<String>
)
