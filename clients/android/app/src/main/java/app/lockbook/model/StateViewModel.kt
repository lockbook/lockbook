package app.lockbook.model

import android.content.ClipData
import android.content.Intent
import android.net.Uri
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.FileProvider
import androidx.fragment.app.Fragment
import androidx.fragment.app.FragmentManager
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
import java.io.File

class StateViewModel: ViewModel() {
    val openedFile: ClientFileMetadata? = null
    val transientScreen: TransientScreen? = null

    private val _launchDetailsScreen = SingleMutableLiveData<DetailsScreen>()
    private val _launchDialogScreen = SingleMutableLiveData<TransientScreen>()
    private val _updateMainScreenUI = SingleMutableLiveData<UpdateMainScreenUI>()

    val launchDetailsScreen: LiveData<DetailsScreen>
        get() = _launchDetailsScreen

    val launchTransientScreen: LiveData<TransientScreen>
        get() = _launchDialogScreen

    val updateMainScreenUI: LiveData<UpdateMainScreenUI>
        get() = _updateMainScreenUI

    private val shareModel = ShareModel(_updateMainScreenUI)

    fun shareFiles(files: List<ClientFileMetadata>, cacheDir: File) {
        viewModelScope.launch(Dispatchers.IO) {
            shareModel.shareDocuments(files, cacheDir)
        }
    }

    fun sync(

    )
}

enum class DetailsScreen {
    Blank,
    TextEditor,
    Drawing,
}

sealed class TransientScreen {
    data class Move(val ids: Array<String>): TransientScreen()
    data class Rename(val file: ClientFileMetadata): TransientScreen()
    data class Create(val info: CreateFileInfo): TransientScreen()
    data class Info(val file: ClientFileMetadata): TransientScreen()
    data class Share(val files: List<File>): TransientScreen()

    private fun show(fragment: Fragment) {
        when(this) {
            is Share -> {
                showShare(fragment)
            }
            is Create -> {
                CreateFileDialogFragment().show(fragment.parentFragmentManager, CreateFileDialogFragment.CREATE_FILE_DIALOG_TAG)
            }
            is Info -> {
                FileInfoDialogFragment().show(fragment.parentFragmentManager, FileInfoDialogFragment.FILE_INFO_DIALOG_TAG)
            }
            is Move -> {
                MoveFileDialogFragment().show(fragment.parentFragmentManager, MoveFileDialogFragment.MOVE_FILE_DIALOG_TAG)
            }
            is Rename -> {
                RenameFileDialogFragment().show(fragment.parentFragmentManager, RenameFileDialogFragment.RENAME_FILE_DIALOG_TAG)
            }
        }
    }

    private fun Share.showShare(fragment: Fragment) {
        val onShare =
            fragment.registerForActivityResult(ActivityResultContracts.StartActivityForResult()) {

            }

        val uris = java.util.ArrayList<Uri>()

        for (file in files) {
            uris.add(
                FileProvider.getUriForFile(
                    fragment.requireContext(),
                    "app.lockbook.fileprovider",
                    file
                )
            )
        }

        val intent = Intent(Intent.ACTION_SEND_MULTIPLE)
        intent.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)

        val clipData = ClipData.newRawUri(null, Uri.EMPTY)
        uris.forEach { uri ->
            clipData.addItem(ClipData.Item(uri))
        }

        intent.clipData = clipData
        intent.type = "*/*"
        intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        intent.putParcelableArrayListExtra(Intent.EXTRA_STREAM, uris)

        onShare.launch(
            Intent.createChooser(
                intent,
                "Send multiple files."
            )
        )
    }
}

sealed class UpdateMainScreenUI {
    data class ShareDocuments(val files: ArrayList<File>): UpdateMainScreenUI()
    data class ShowHideProgressOverlay(val hide: Boolean): UpdateMainScreenUI()
    data class NotifyError(val error: LbError): UpdateMainScreenUI()
}

data class CreateFileInfo(
    val parentId: String,
    val fileType: FileType,
    val isDrawing: Boolean
)

data class MoveFileInfo(
    val ids: Array<String>,
    val names: Array<String>
)