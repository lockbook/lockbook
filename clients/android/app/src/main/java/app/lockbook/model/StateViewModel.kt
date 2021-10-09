package app.lockbook.model

import android.content.ClipData
import android.content.Intent
import android.net.Uri
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.FileProvider
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.LiveData
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.lockbook.ui.CreateFileDialogFragment
import app.lockbook.ui.FileInfoDialogFragment
import app.lockbook.ui.MoveFileDialogFragment
import app.lockbook.ui.RenameFileDialogFragment
import app.lockbook.util.ClientFileMetadata
import app.lockbook.util.FileType
import app.lockbook.util.LbError
import app.lockbook.util.SingleMutableLiveData
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import timber.log.Timber
import java.io.File

class StateViewModel: ViewModel() {
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

    fun launchTransientScreen(screen: TransientScreen) {
        transientScreen = screen
        _launchTransientScreen.postValue(transientScreen)
    }

    fun launchDetailsScreen(screen: DetailsScreen) {
        detailsScreen = screen
        _launchDetailsScreen.postValue(detailsScreen)
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

    fun show(activity: FragmentActivity) {
        when(this) {
            is Create -> {
                CreateFileDialogFragment().show(activity.supportFragmentManager, CreateFileDialogFragment.CREATE_FILE_DIALOG_TAG)
            }
            is Info -> {
                FileInfoDialogFragment().show(activity.supportFragmentManager, FileInfoDialogFragment.FILE_INFO_DIALOG_TAG)
            }
            is Move -> {
                MoveFileDialogFragment().show(activity.supportFragmentManager, MoveFileDialogFragment.MOVE_FILE_DIALOG_TAG)
            }
            is Rename -> {
                RenameFileDialogFragment().show(activity.supportFragmentManager, RenameFileDialogFragment.RENAME_FILE_DIALOG_TAG)
            }
        }
    }
}

sealed class UpdateMainScreenUI {
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