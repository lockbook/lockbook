package app.lockbook.screen

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import app.lockbook.databinding.FragmentImageViewerBinding
import app.lockbook.model.DetailsScreen
import app.lockbook.model.StateViewModel

class ImageViewerFragment : Fragment() {
    private var _binding: FragmentImageViewerBinding? = null
    private val binding get() = _binding!!

    private val activityModel: StateViewModel by activityViewModels()

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = FragmentImageViewerBinding.inflate(inflater, container, false)
        val imageViewerInfo = activityModel.detailsScreen as DetailsScreen.ImageViewer

        binding.imageViewToolbar.title = imageViewerInfo.fileMetadata.decryptedName
        binding.imageViewer.setImageBitmap(imageViewerInfo.bitMap)

        return binding.root
    }
}
