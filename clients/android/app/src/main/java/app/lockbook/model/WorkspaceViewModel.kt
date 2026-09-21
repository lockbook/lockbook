@file:Suppress(
    "ktlint:standard:backing-property-naming",
    "ktlint:standard:property-naming",
)

package app.lockbook.model

import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import app.lockbook.util.SingleMutableLiveData
import app.lockbook.util.StagedAttachment
import app.lockbook.workspace.NULL_UUID
import com.afollestad.recyclical.datasource.emptyDataSourceTyped
import net.lockbook.File
import java.util.ArrayDeque

class WorkspaceViewModel : ViewModel() {
    /** request workspace to  open a file **/
    private val _openFile = SingleMutableLiveData<OpenFileRequest>()
    val openFile: LiveData<OpenFileRequest>
        get() = _openFile

    /** request workspace to  close a file **/
    val _closeFile = SingleMutableLiveData<String>()
    val closeFile: LiveData<String>
        get() = _closeFile

    val _currentTab = MutableLiveData<WorkspaceTab>()
    val currentTab: LiveData<WorkspaceTab>
        get() = _currentTab

    var isFileTreeSyncedToCurrentTab = true

    val _finishedAction = SingleMutableLiveData<FinishedAction>()
    val finishedAction: LiveData<FinishedAction>
        get() = _finishedAction

    val _hideToolbar = SingleMutableLiveData<Float>()
    val hideToolbar: LiveData<Float>
        get() = _hideToolbar

    var tabs = emptyDataSourceTyped<OpenTab>()

    val _keyboardVisible = MutableLiveData<Boolean>()
    val keyboardVisible: LiveData<Boolean>
        get() = _keyboardVisible

    val _nativeMarkdownToolbarVisible = MutableLiveData(false)
    val nativeMarkdownToolbarVisible: LiveData<Boolean>
        get() = _nativeMarkdownToolbarVisible

    val _showKeyboard = MutableLiveData<Boolean>()
    val showKeyboard: LiveData<Boolean>
        get() = _showKeyboard

    val _tabListExpanded = MutableLiveData(false)
    val tabListExpanded: LiveData<Boolean>
        get() = _tabListExpanded

    val _bottomInset = MutableLiveData<Int>()
    val bottomInset: LiveData<Int>
        get() = _bottomInset

    /** pull up the photo source sheet so the user can import a pic or take one */
    val _photoSourceRequested = SingleMutableLiveData<Unit>()
    val photoSourceRequested: LiveData<Unit>
        get() = _photoSourceRequested

    /** Holds staged files until the workspace copies their bytes into a paste event. */
    private val pendingAttachments = ArrayDeque<StagedAttachment>()

    /** request workspace view to navigate within tab history **/
    private val _workspaceBackRequested = SingleMutableLiveData<Unit>()
    val workspaceBackRequested: LiveData<Unit>
        get() = _workspaceBackRequested

    private val _backGestureStarted = SingleMutableLiveData<Unit>()
    val backGestureStarted: LiveData<Unit>
        get() = _backGestureStarted

    /** request workspace view to navigate forward within tab history **/
    private val _workspaceForwardRequested = SingleMutableLiveData<Unit>()
    val workspaceForwardRequested: LiveData<Unit>
        get() = _workspaceForwardRequested

    fun requestWorkspaceBack() {
        _workspaceBackRequested.postValue(Unit)
    }

    fun notifyBackGestureStarted() {
        _backGestureStarted.postValue(Unit)
    }

    internal fun enqueueAttachment(attachment: StagedAttachment) {
        pendingAttachments.addLast(attachment)
    }

    internal fun nextAttachment(): StagedAttachment? = pendingAttachments.firstOrNull()

    /** Removes the file after native code has copied it (or rejected it). */
    internal fun removeNextAttachment(): StagedAttachment? = pendingAttachments.pollFirst()

    fun openFile(request: OpenFileRequest) {
        _openFile.value = request
    }

    fun postOpenFile(request: OpenFileRequest) {
        _openFile.postValue(request)
    }
}
    
data class OpenFileRequest(
    val id: String,
    val newFile: Boolean,
    val presentation: OpenFilePresentation,
)

enum class OpenFilePresentation {
    Preserve,
    ShowDetail,
}

data class WorkspaceTab(
    val id: String,
    val type: WorkspaceTabType,
    val sessionId: String = NULL_UUID,
) {
    companion object {
        // Helper to represent the "empty" or default welcome state
        val welcome = WorkspaceTab(NULL_UUID, WorkspaceTabType.Welcome)
    }
}

data class OpenTab(
    val sessionId: String,
    val file: File,
)

enum class WorkspaceTabType(
    val value: Int,
) {
    Welcome(0),
    Loading(1),
    Image(2),
    Markdown(3),
    PlainText(4),
    Pdf(5),
    Svg(6),
    Graph(7),
    Chat(9),
    ;

    companion object {
        fun fromInt(value: Int): WorkspaceTabType? = WorkspaceTabType.entries.find { it.value == value }
    }

    fun isTextEdit(): Boolean = this == Markdown || this == PlainText || this == Chat
}

sealed class FinishedAction {
    data class Delete(
        val id: String,
    ) : FinishedAction()

    data class Rename(
        val id: String,
        val name: String,
    ) : FinishedAction()
}
