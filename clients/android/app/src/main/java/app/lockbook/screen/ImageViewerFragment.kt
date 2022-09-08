package app.lockbook.screen

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import app.lockbook.databinding.FragmentImageViewerBinding
import app.lockbook.model.DetailScreen
import app.lockbook.model.StateViewModel
import app.lockbook.util.Animate

class ImageViewerFragment : Fragment() {
    private var _binding: FragmentImageViewerBinding? = null
    private val binding get() = _binding!!

    private val activityModel: StateViewModel by activityViewModels()
    private var isToolbarVisible = true

    companion object {
        const val TOOLBAR_ALPHA = 100
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = FragmentImageViewerBinding.inflate(inflater, container, false)
        val imageViewerInfo = activityModel.detailsScreen as DetailScreen.ImageViewer

        setUpImageAndToolbar(imageViewerInfo, savedInstanceState)

        return binding.root
    }

    private fun setUpImageAndToolbar(
        imageViewerInfo: DetailScreen.ImageViewer,
        savedInstanceState: Bundle?
    ) {
        binding.imageViewToolbar.title = imageViewerInfo.file.name
        binding.imageViewToolbar.background.alpha = TOOLBAR_ALPHA
        binding.imageViewToolbar.setNavigationOnClickListener {
            activityModel.launchDetailsScreen(null)
        }
        binding.imageViewer.setImageBitmap(imageViewerInfo.bitmap)
        binding.imageViewer.maxZoom = 7f
        binding.imageViewer.setOnClickListener {
            if (isToolbarVisible) {
                isToolbarVisible = false
                Animate.animateVisibility(binding.imageViewToolbar, View.GONE, 0, 200)
            } else {
                isToolbarVisible = true
                Animate.animateVisibility(binding.imageViewToolbar, View.VISIBLE, TOOLBAR_ALPHA, 200)
            }
        }

        if (savedInstanceState?.getBoolean(IS_TOOLBAR_VISIBLE_KEY) == false) {
            isToolbarVisible = false
            Animate.animateVisibility(binding.imageViewToolbar, View.GONE, 0, 200)
        }
    }

    override fun onSaveInstanceState(outState: Bundle) {
        outState.putBoolean(IS_TOOLBAR_VISIBLE_KEY, isToolbarVisible)
        super.onSaveInstanceState(outState)
    }
}
