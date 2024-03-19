package app.lockbook.model

import android.net.Uri
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import app.lockbook.util.SingleMutableLiveData

class WorkspaceViewModel: ViewModel() {
    // for workspace fragment
    val _openFile = SingleMutableLiveData<Pair<String, Boolean>>()
    val openFile: LiveData<Pair<String, Boolean>>
        get() = _openFile

    val _closeDocument = SingleMutableLiveData<String>()
    val closeDocument: LiveData<String>
        get() = _closeDocument

    val _sync = SingleMutableLiveData<Unit>()
    val sync: LiveData<Unit>
        get() = _sync

    var isSyncing = false

    // for everyone else
    val _msg = MutableLiveData<String>()
    val msg: LiveData<String>
        get() = _msg

    val _selectedFile = MutableLiveData<String>()
    val selectedFile: LiveData<String>
        get() = _selectedFile

    val _docCreated = MutableLiveData<String>()
    val docCreated: LiveData<String>
        get() = _docCreated

    val _refreshFiles = SingleMutableLiveData<Unit>()
    val refreshFiles: LiveData<Unit>
        get() = _refreshFiles

    val _newFolderBtnPressed = SingleMutableLiveData<Unit>()
    val newFolderBtnPressed: LiveData<Unit>
        get() = _newFolderBtnPressed

    val _tabTitleClicked = SingleMutableLiveData<Unit>()
    val tabTitleClicked: LiveData<Unit>
        get() = _tabTitleClicked

    val _syncCompleted = SingleMutableLiveData<Unit>()
    val syncCompleted: LiveData<Unit>
        get() = _syncCompleted
}