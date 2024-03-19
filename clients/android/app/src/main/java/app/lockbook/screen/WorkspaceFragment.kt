package app.lockbook.screen

import android.net.Uri
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import app.lockbook.databinding.FragmentWorkspaceBinding
import app.lockbook.model.StateViewModel
import app.lockbook.model.UpdateMainScreenUI
import app.lockbook.model.UpdateSearchUI
import app.lockbook.util.SingleMutableLiveData
import timber.log.Timber

class WorkspaceFragment: Fragment() {
    private var _binding: FragmentWorkspaceBinding? = null
    private val binding get() = _binding!!

    private val activityModel: StateViewModel by activityViewModels()
    private val model: WorkspaceViewModel by viewModels()

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = FragmentWorkspaceBinding.inflate(inflater, container, false)

        binding.workspaceToolbar.setNavigationOnClickListener {
            activityModel.updateMainScreenUI(UpdateMainScreenUI.OpenFile(null))
        }

        binding.workspace.stateModel = model

        return binding.root
    }
}

class WorkspaceViewModel: ViewModel() {
    // for workspace fragment
    val _openFile = SingleMutableLiveData<String>()
    val openFile: LiveData<String>
        get() = _openFile

    val _closeDocument = SingleMutableLiveData<Uri>()
    val closeDocument: LiveData<Uri>
        get() = _closeDocument

    val _sync = SingleMutableLiveData<Unit>()
    val sync: LiveData<Unit>
        get() = _sync

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

}