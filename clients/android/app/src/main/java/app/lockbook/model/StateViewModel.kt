package app.lockbook.model

import android.app.Application
import android.content.ClipData
import android.content.Intent
import android.net.Uri
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.FileProvider
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.lockbook.getRes
import app.lockbook.screen.UpdateFilesUI
import app.lockbook.ui.CreateFileDialogFragment
import app.lockbook.ui.FileInfoDialogFragment
import app.lockbook.ui.MoveFileDialogFragment
import app.lockbook.ui.RenameFileDialogFragment
import app.lockbook.util.ClientFileMetadata
import app.lockbook.util.FileType
import app.lockbook.util.LbError
import app.lockbook.util.SingleMutableLiveData
import com.github.michaelbull.result.Err
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import timber.log.Timber
import java.io.File
import java.util.ArrayList

class StateViewModel(application: Application): AndroidViewModel(application) {
    var detailsScreen: DetailsScreen = DetailsScreen.Blank
    var transientScreen: TransientScreen? = null

    val _launchDetailsScreen = SingleMutableLiveData<DetailsScreen>()
    private val _launchTransientScreen = SingleMutableLiveData<TransientScreen>()
    private val _updateMainScreenUI = SingleMutableLiveData<UpdateMainScreenUI>()

    val launchDetailsScreen: LiveData<DetailsScreen>
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

    fun launchDetailsScreen(screen: DetailsScreen) {
        detailsScreen = screen
        _launchDetailsScreen.postValue(detailsScreen)
    }

    fun shareSelectedFiles(selectedFiles: List<ClientFileMetadata>, appDataDir: File) {
        viewModelScope.launch(Dispatchers.IO) {
            val shareResult = shareModel.shareDocuments(selectedFiles, appDataDir)
            if(shareResult is Err) {
                _updateMainScreenUI.postValue(UpdateMainScreenUI.NotifyError(shareResult.error.toLbError(getRes())))
                return@launch
            }
        }
    }
}

sealed class DetailsScreen {
    object Blank: DetailsScreen()
    data class TextEditor(val fileMetadata: ClientFileMetadata): DetailsScreen()
    data class Drawing(val fileMetadata: ClientFileMetadata): DetailsScreen()
}

sealed class TransientScreen {
    data class Move(val ids: Array<String>): TransientScreen()
    data class Rename(val file: ClientFileMetadata): TransientScreen()
    data class Create(val info: CreateFileInfo): TransientScreen()
    data class Info(val file: ClientFileMetadata): TransientScreen()
    data class Share(val files: List<File>): TransientScreen()
}

sealed class UpdateMainScreenUI {
    data class ShowHideProgressOverlay(val show: Boolean) : UpdateMainScreenUI()
    data class ShareDocuments(val files: ArrayList<File>) : UpdateMainScreenUI()
    data class NotifyError(val error: LbError): UpdateMainScreenUI()
}

data class CreateFileInfo(
    val parentId: String,
    val extendedFileType: ExtendedFileType
)

sealed class ExtendedFileType{
    object Text: ExtendedFileType()
    object Drawing: ExtendedFileType()
    object Folder: ExtendedFileType()

    fun toFileType(): FileType = when(this) {
        Drawing, Text -> FileType.Document
        Folder -> FileType.Folder
    }
}

data class MoveFileInfo(
    val ids: Array<String>,
    val names: Array<String>
)