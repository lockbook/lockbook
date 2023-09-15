package app.lockbook.model

import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import app.lockbook.util.SingleMutableLiveData
import app.lockbook.workspace.NULL_UUID
import com.afollestad.recyclical.datasource.emptyDataSourceTyped
import net.lockbook.File

class WorkspaceViewModel : ViewModel() {

    /** request workspace to  open a file **/
    val _openFile = SingleMutableLiveData<Pair<String, Boolean>>()
    val openFile: LiveData<Pair<String, Boolean>>
        get() = _openFile

    /** request workspace to  close a file **/
    val _closeFile = SingleMutableLiveData<String>()
    val closeFile: LiveData<String>
        get() = _closeFile


    /** request workspace to create a new file (isDrawing, parentId) **/
    val _createDocAt = MutableLiveData<Pair<Boolean, String>>()
    val createDocAt: LiveData<Pair<Boolean, String>>
        get() = _createDocAt

    val _currentTab = MutableLiveData<WorkspaceTab>()
    val currentTab: LiveData<WorkspaceTab>
        get() = _currentTab

    val _finishedAction = SingleMutableLiveData<FinishedAction>()
    val finishedAction: LiveData<FinishedAction>
        get() = _finishedAction

    val _hideToolbar = SingleMutableLiveData<Float>()
    val hideToolbar: LiveData<Float>
        get() = _hideToolbar

    val _tabTitleClicked = SingleMutableLiveData<Unit>()
    val tabTitleClicked: LiveData<Unit>
        get() = _tabTitleClicked

    val _refreshFilesRequested = SingleMutableLiveData<Unit>()
    val refreshFilesRequested: LiveData<Unit>
        get() = _refreshFilesRequested

    var tabs = emptyDataSourceTyped<File>()

    val _keyboardVisible = MutableLiveData<Boolean>()
    val keyboardVisible: LiveData<Boolean>
        get() = _keyboardVisible

    val _showKeyboard = MutableLiveData<Boolean>()
    val showKeyboard: LiveData<Boolean>
        get() = _showKeyboard

    val _tabListExpanded = MutableLiveData(false)
    val tabListExpanded: LiveData<Boolean>
        get() = _tabListExpanded

    val _bottomInset = MutableLiveData<Int>()
    val bottomInset: LiveData<Int>
        get() = _bottomInset

    val _fps = MutableLiveData<Float>()
    val fps: LiveData<Float>
        get() = _fps

    /** request workspace view to navigate within tab history **/
    private val _workspaceBackRequested = SingleMutableLiveData<Unit>()
    val workspaceBackRequested: LiveData<Unit>
        get() = _workspaceBackRequested

    /** request workspace view to navigate forward within tab history **/
    private val _workspaceForwardRequested = SingleMutableLiveData<Unit>()
    val workspaceForwardRequested: LiveData<Unit>
        get() = _workspaceForwardRequested

    fun requestWorkspaceBack() {
        _workspaceBackRequested.postValue(Unit)
    }

    fun requestWorkspaceForward() {
        _workspaceForwardRequested.postValue(Unit)
    }
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
