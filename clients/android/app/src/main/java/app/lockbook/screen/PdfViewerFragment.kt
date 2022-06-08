package app.lockbook.screen

import android.content.res.Configuration
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import app.lockbook.databinding.FragmentPdfViewerBinding
import app.lockbook.model.DetailsScreen
import app.lockbook.model.OPENED_FILE_FOLDER
import app.lockbook.model.StateViewModel
import com.github.barteksc.pdfviewer.link.DefaultLinkHandler
import java.io.File

class PdfViewerFragment : Fragment() {
    private var _binding: FragmentPdfViewerBinding? = null
    private val binding get() = _binding!!

    private val activityModel: StateViewModel by activityViewModels()
    private lateinit var fileName: String

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = FragmentPdfViewerBinding.inflate(inflater, container, false)
        val pdfViewerInfo = activityModel.detailsScreen as DetailsScreen.PdfViewer
        fileName = pdfViewerInfo.fileMetadata.decryptedName

        binding.pdfViewToolbar.title = fileName
        binding.pdfViewer.fromFile(File(pdfViewerInfo.location, fileName))
            .enableDoubletap(true)
            .enableAnnotationRendering(true)
            .enableAntialiasing(true)
            .linkHandler(DefaultLinkHandler(binding.pdfViewer))
            .nightMode((resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES)
            .load()

        return binding.root
    }

    fun deleteLocalPdfInstance() {
        File(requireContext().cacheDir, OPENED_FILE_FOLDER + fileName).delete()
    }
}
