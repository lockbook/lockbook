package app.lockbook.model

import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import app.lockbook.util.SingleMutableLiveData
import app.lockbook.workspace.NULL_UUID
import com.afollestad.recyclical.datasource.emptyDataSourceTyped
import net.lockbook.File

class WorkspaceViewModel : ViewModel() {

    var isSyncing = false

    /** request workspace to  open a file **/
    val _openFile = SingleMutableLiveData<Pair<String, Boolean>>()
    val openFile: LiveData<Pair<String, Boolean>>
        get() = _openFile

    /** request workspace to  close a file **/
    val _closeFile = SingleMutableLiveData<String>()
    val closeFile: LiveData<String>
        get() = _closeFile

    /** request workspace to sync **/
    val _sync = SingleMutableLiveData<Unit>()
    val sync: LiveData<Unit>
        get() = _sync

    var lastSyncStatusUpdate = System.currentTimeMillis()

    /** are tabs shown in workspace **/
    val _showTabs = SingleMutableLiveData<Boolean>()
    val showTabs: LiveData<Boolean>
        get() = _showTabs

    /** request workspace to show tabs **/
    val _shouldShowTabs = SingleMutableLiveData<Unit>()
    val shouldShowTabs: LiveData<Unit>
        get() = _shouldShowTabs

    /** request workspace to create a new file **/
    val _createFile = MutableLiveData<String>()
    val createFile: LiveData<String>
        get() = _createFile

    val _currentTab = MutableLiveData<WorkspaceTab>(WorkspaceTab.Welcome)
    val currentTab: LiveData<WorkspaceTab>
        get() = _currentTab

    val _finishedAction = SingleMutableLiveData<FinishedAction>()
    val finishedAction: LiveData<FinishedAction>
        get() = _finishedAction

    // for everyone else
    val _msg = MutableLiveData<String>()
    val msg: LiveData<String>
        get() = _msg

    val _refreshFiles = SingleMutableLiveData<Unit>()
    val refreshFiles: LiveData<Unit>
        get() = _refreshFiles

    val _hideMaterialToolbar = SingleMutableLiveData<Float>()
    val hideMaterialToolbar: LiveData<Float>
        get() = _hideMaterialToolbar

    val _newFolderBtnPressed = SingleMutableLiveData<Unit>()
    val newFolderBtnPressed: LiveData<Unit>
        get() = _newFolderBtnPressed

    val _tabTitleClicked = SingleMutableLiveData<Unit>()
    val tabTitleClicked: LiveData<Unit>
        get() = _tabTitleClicked

    val _syncCompleted = SingleMutableLiveData<Unit>()
    val syncCompleted: LiveData<Unit>
        get() = _syncCompleted

    var tabs = emptyDataSourceTyped<File>()

    val _keyboardVisible = MutableLiveData<Boolean>()
    val keyboardVisible: LiveData<Boolean>
        get() = _keyboardVisible

    val _bottomSheetExpanded = MutableLiveData<Boolean>(false)
    val bottomSheetExpanded: LiveData<Boolean>
        get() = _bottomSheetExpanded

    val _bottomInset = MutableLiveData<Int>()
    val bottomInset: LiveData<Int>
        get() = _bottomInset

    val _isRendering = MutableLiveData<Boolean>()
    val isRendering: LiveData<Boolean>
        get() = _isRendering
}

data class WorkspaceTab(
    val id: String,
    val type: WorkspaceTabType
) {
    companion object {
        // Helper to represent the "empty" or default welcome state
        val Welcome = WorkspaceTab(NULL_UUID, WorkspaceTabType.Welcome)
    }
}
enum class WorkspaceTabType(val value: Int) {
    Welcome(0),
    Loading(1),
    Image(2),
    Markdown(3),
    PlainText(4),
    Pdf(5),
    Svg(6),
    Graph(7);

    companion object {
        fun fromInt(value: Int): WorkspaceTabType? {
            return WorkspaceTabType.entries.find { it.value == value }
        }
    }

    fun viewWrapperId(): Int {
        return when (this) {
            Welcome, Pdf, Loading, Image, Graph -> 1
            Svg -> 2
            PlainText, Markdown -> 3
        }
    }

    fun isTextEdit(): Boolean {
        return this == Markdown || this == PlainText
    }

    fun isSvg(): Boolean {
        return this == Svg
    }
}

sealed class FinishedAction {
    data class Delete(val id: String) : FinishedAction()
    data class Rename(val id: String, val name: String) : FinishedAction()
}
