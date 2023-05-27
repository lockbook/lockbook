package app.lockbook.model

import android.app.Application
import android.graphics.Bitmap
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import java.io.File

class StateViewModel(application: Application) : AndroidViewModel(application) {
    var activityScreen: ActivityScreen? = null
    var detailScreen: DetailScreen? = null
    var transientScreen: TransientScreen? = null

    private val _launchActivityScreen = SingleMutableLiveData<ActivityScreen>()
    private val _launchDetailScreen = SingleMutableLiveData<DetailScreen?>()
    private val _launchTransientScreen = SingleMutableLiveData<TransientScreen>()
    private val _updateMainScreenUI = SingleMutableLiveData<UpdateMainScreenUI>()

    val launchActivityScreen: LiveData<ActivityScreen?>
        get() = _launchActivityScreen

    val launchDetailScreen: LiveData<DetailScreen?>
        get() = _launchDetailScreen

    val launchTransientScreen: LiveData<TransientScreen>
        get() = _launchTransientScreen

    val updateMainScreenUI: LiveData<UpdateMainScreenUI>
        get() = _updateMainScreenUI

    val exportImportModel = ExportImportModel(_updateMainScreenUI)
    val syncModel = SyncModel()

    fun launchActivityScreen(screen: ActivityScreen) {
        activityScreen = screen
        _launchActivityScreen.postValue(activityScreen)
    }

    fun launchTransientScreen(screen: TransientScreen) {
        transientScreen = screen
        _launchTransientScreen.postValue(transientScreen)
    }

    fun launchDetailScreen(screen: DetailScreen?) {
        detailScreen = screen
        _launchDetailScreen.value = detailScreen
    }

    fun updateMainScreenUI(uiUpdate: UpdateMainScreenUI) {
        _updateMainScreenUI.postValue(uiUpdate)
    }

    fun shareSelectedFiles(selectedFiles: List<app.lockbook.util.File>, appDataDir: File) {
        viewModelScope.launch(Dispatchers.IO) {
            val exportResult = exportImportModel.exportDocuments(selectedFiles, appDataDir)
            if (exportResult is Err) {
                _updateMainScreenUI.postValue(
                    UpdateMainScreenUI.NotifyError(
                        exportResult.error.toLbError(
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
            val saveDrawingResult =
                CoreModel.saveDrawing(id, drawing)

            if (saveDrawingResult is Err) {
                _updateMainScreenUI.postValue(
                    UpdateMainScreenUI.NotifyError(
                        saveDrawingResult.error.toLbError(
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

    fun confirmSubscription(purchaseToken: String, accountId: String) {
        viewModelScope.launch(Dispatchers.IO) {
            val confirmSubscriptionResult =
                CoreModel.upgradeAccountGooglePlay(purchaseToken, accountId)

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

sealed class ActivityScreen {
    data class Settings(val scrollToPreference: Int? = null) : ActivityScreen()
    object Shares : ActivityScreen()
}

sealed class DetailScreen {
    data class Loading(val file: app.lockbook.util.File) : DetailScreen()
    data class TextEditor(val file: app.lockbook.util.File, val text: String) :
        DetailScreen()

    data class Drawing(
        val file: app.lockbook.util.File,
        val drawing: app.lockbook.util.Drawing
    ) : DetailScreen()

    data class ImageViewer(
        val file: app.lockbook.util.File,
        val bitmap: Bitmap
    ) : DetailScreen()

    data class PdfViewer(
        val file: app.lockbook.util.File,
        val location: File
    ) : DetailScreen()

    data class Share(val file: app.lockbook.util.File) : DetailScreen()

    fun getUsedFile(): app.lockbook.util.File = when (this) {
        is Drawing -> file
        is ImageViewer -> file
        is Loading -> file
        is PdfViewer -> file
        is TextEditor -> file
        is Share -> file
    }
}

sealed class TransientScreen {
    data class Move(val files: List<app.lockbook.util.File>) : TransientScreen()
    data class Rename(val file: app.lockbook.util.File) : TransientScreen()
    data class Create(val parentId: String, val extendedFileType: ExtendedFileType) : TransientScreen()
    data class Info(val file: app.lockbook.util.File) : TransientScreen()
    data class ShareExport(val files: List<File>) : TransientScreen()
    data class Delete(val files: List<app.lockbook.util.File>) : TransientScreen()
}

sealed class UpdateMainScreenUI {
    data class ShowHideProgressOverlay(val show: Boolean) : UpdateMainScreenUI()
    data class ShareDocuments(val files: ArrayList<File>) : UpdateMainScreenUI()
    data class NotifyError(val error: LbError) : UpdateMainScreenUI()
    object ShowSubscriptionConfirmed : UpdateMainScreenUI()
    object ShowSearch : UpdateMainScreenUI()
    object ShowFiles : UpdateMainScreenUI()
    object Sync : UpdateMainScreenUI()
}

sealed class ExtendedFileType {
    object Document : ExtendedFileType()
    object Drawing : ExtendedFileType()
    object Folder : ExtendedFileType()

    fun toFileType(): FileType = when (this) {
        Drawing, Document -> FileType.Document
        Folder -> FileType.Folder
    }
}
