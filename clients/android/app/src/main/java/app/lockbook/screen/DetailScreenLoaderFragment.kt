package app.lockbook.screen

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import app.lockbook.databinding.FragmentLoadingScreenBinding
import app.lockbook.model.*
import java.io.File
import java.lang.ref.WeakReference

class DetailScreenLoaderFragment : Fragment() {
    private var _binding: FragmentLoadingScreenBinding? = null
    private val binding get() = _binding!!

    private val activityModel: StateViewModel by activityViewModels()
    private val model: DetailScreenLoaderViewModel by viewModels(
        factoryProducer = {
            object : ViewModelProvider.Factory {
                override fun <T : ViewModel> create(modelClass: Class<T>): T {
                    if (modelClass.isAssignableFrom(DetailScreenLoaderViewModel::class.java))
                        return DetailScreenLoaderViewModel(
                            requireActivity().application,
                            activityModel.detailScreen as DetailScreen.Loading
                        ) as T
                    throw IllegalArgumentException("Unknown ViewModel class")
                }
            }
        }
    )

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    fun addChecker() {
        model.updateDetailScreenLoaderUI.observe(viewLifecycleOwner) {
            when (it) {
                is UpdateDetailScreenLoaderUI.NotifyError -> alertModel.notifyError(it.error)
                is UpdateDetailScreenLoaderUI.NotifyFinished -> activityModel.launchDetailScreen(it.newScreen)
            }
        }
    }

    fun deleteDownloadedFileIfExists() {
        File(requireContext().cacheDir, OPENED_FILE_FOLDER + model.loadingInfo.file.name).delete()
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = FragmentLoadingScreenBinding.inflate(inflater, container, false)

        if (!(activity as MainScreenActivity).binding.slidingPaneLayout.isSlideable) {
            addChecker()
        }

        return binding.root
    }
}

const val IS_TOOLBAR_VISIBLE_KEY = "is_toolbar_visible_key"
