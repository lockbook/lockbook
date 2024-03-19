package app.lockbook.screen

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import app.lockbook.databinding.FragmentWorkspaceBinding
import app.lockbook.model.StateViewModel
import app.lockbook.model.UpdateMainScreenUI
import app.lockbook.model.WorkspaceViewModel

class WorkspaceFragment: Fragment() {
    private var _binding: FragmentWorkspaceBinding? = null
    private val binding get() = _binding!!

    private val activityModel: StateViewModel by activityViewModels()
    private val model: WorkspaceViewModel by activityViewModels()

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

        model.sync.observe(viewLifecycleOwner) {
            binding.workspace.sync()
        }

        model.openFile.observe(viewLifecycleOwner) { (id, newFile) ->
            binding.workspace.openDoc(id, newFile)
        }

        model.docCreated.observe(viewLifecycleOwner) { id ->
            binding.workspace.openDoc(id, true)
        }

        model.closeDocument.observe(viewLifecycleOwner) { id ->
            binding.workspace.closeDoc(id)
        }

        return binding.root
    }
}
