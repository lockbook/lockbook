package app.lockbook.model

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.util.*
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import net.lockbook.Lb
import net.lockbook.LbError
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

    fun shareSelectedFiles(selectedFiles: List<net.lockbook.File>, appDataDir: File) {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                exportImportModel.exportDocuments(selectedFiles, appDataDir)
            } catch (err: LbError) {
                _updateMainScreenUI.postValue(UpdateMainScreenUI.NotifyError(err))
            }
        }
    }

    fun confirmSubscription(purchaseToken: String, accountId: String) {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                Lb.upgradeAccountGooglePlay(purchaseToken, accountId)
                _updateMainScreenUI.postValue(UpdateMainScreenUI.ShowSubscriptionConfirmed)
            } catch (err: LbError) {
                _updateMainScreenUI.postValue(
                    UpdateMainScreenUI.NotifyError(err)
                )
            }
        }
    }
}

sealed class ActivityScreen {
    data class Settings(val scrollToPreference: Int? = null) : ActivityScreen()
}

sealed class TransientScreen {
    data class Move(val files: List<net.lockbook.File>) : TransientScreen()
    data class Rename(val file: net.lockbook.File) : TransientScreen()
    data class Create(val parentId: String) : TransientScreen()
    data class Info(val file: net.lockbook.File) : TransientScreen()
    data class ShareExport(val files: List<File>) : TransientScreen()
    data class ShareFile(val file: net.lockbook.File) : TransientScreen()
    data class Delete(val files: List<net.lockbook.File>) : TransientScreen()
}

sealed class UpdateMainScreenUI {
    data class OpenFile(val id: String?) : UpdateMainScreenUI()



    data class ShowHideProgressOverlay(val show: Boolean) : UpdateMainScreenUI()
    data class ShareDocuments(val files: ArrayList<File>) : UpdateMainScreenUI()
    data class NotifyError(val error: LbError) : UpdateMainScreenUI()
    object ShowSubscriptionConfirmed : UpdateMainScreenUI()
    object ShowSearch : UpdateMainScreenUI()
    object ShowFiles : UpdateMainScreenUI()
    object PopBackstackToWorkspace : UpdateMainScreenUI()
    object ToggleBottomViewNavigation : UpdateMainScreenUI()
    object CloseSlidingPane : UpdateMainScreenUI()
    object CloseWorkspaceDoc : UpdateMainScreenUI()
    object Sync : UpdateMainScreenUI()
}
