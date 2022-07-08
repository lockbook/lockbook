package app.lockbook.screen

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import app.lockbook.databinding.DialogMoveFileBinding
import app.lockbook.databinding.FragmentSearchFilesBinding
import app.lockbook.model.StateViewModel
import app.lockbook.model.UpdateMainScreenUI
import app.lockbook.util.requestKeyboardFocus
import com.google.android.material.transition.MaterialSharedAxis

class SearchFilesFragment: Fragment() {
    private lateinit var binding: FragmentSearchFilesBinding
    private val activityModel: StateViewModel by activityViewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        binding = FragmentSearchFilesBinding.inflate(layoutInflater)

        binding.searchFilesToolbar.setNavigationOnClickListener {
            activityModel.updateMainScreenUI(UpdateMainScreenUI.ShowFiles)
        }

        return binding.root
    }

}