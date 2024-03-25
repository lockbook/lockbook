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
    var transientScreen: TransientScreen? = null

    private val _launchActivityScreen = SingleMutableLiveData<ActivityScreen>()
    private val _launchTransientScreen = SingleMutableLiveData<TransientScreen>()
    private val _updateMainScreenUI = SingleMutableLiveData<UpdateMainScreenUI>()

    val launchActivityScreen: LiveData<ActivityScreen?>
        get() = _launchActivityScreen

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

sealed class TransientScreen {
    data class Move(val files: List<app.lockbook.util.File>) : TransientScreen()
    data class Rename(val file: app.lockbook.util.File) : TransientScreen()
    data class Create(val parentId: String, val extendedFileType: ExtendedFileType) : TransientScreen()
    data class Info(val file: app.lockbook.util.File) : TransientScreen()
    data class ShareExport(val files: List<File>) : TransientScreen()
    data class Delete(val files: List<app.lockbook.util.File>) : TransientScreen()
}

sealed class UpdateMainScreenUI {
    data class OpenFile(val id: String?): UpdateMainScreenUI()
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
