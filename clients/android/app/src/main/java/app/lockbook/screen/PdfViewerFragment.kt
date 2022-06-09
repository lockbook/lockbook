package app.lockbook.screen

import android.content.res.Configuration
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import app.lockbook.R
import app.lockbook.databinding.FragmentPdfViewerBinding
import app.lockbook.model.DetailsScreen
import app.lockbook.model.OPENED_FILE_FOLDER
import app.lockbook.model.StateViewModel
import app.lockbook.util.Animate
import com.github.barteksc.pdfviewer.link.DefaultLinkHandler
import java.io.File

class PdfViewerFragment : Fragment() {
    private var _binding: FragmentPdfViewerBinding? = null
    private val binding get() = _binding!!

    private var oldPage: Int? = null

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

        oldPage = savedInstanceState?.getInt(PDF_PAGE_KEY)

        binding.pdfViewToolbar.title = fileName
        binding.pdfViewer.fromFile(File(pdfViewerInfo.location, fileName))
            .enableDoubletap(true)
            .enableAnnotationRendering(true)
            .enableAntialiasing(true)
            .linkHandler(DefaultLinkHandler(binding.pdfViewer))
            .nightMode((resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES)
            .defaultPage(oldPage ?: 0)
            .onPageChange { page, pageCount ->
                if (oldPage == 1 && page == 0) {
                    Animate.animateVisibility(binding.pdfViewToolbar, View.VISIBLE, 255, 200)
                } else if (oldPage == 0 && page == 1) {
                    Animate.animateVisibility(binding.pdfViewToolbar, View.GONE, 0, 200)
                }
                binding.pdfPageIndicator.text = getString(R.string.pdf_page_indicator, page + 1, pageCount)
                oldPage = page
            }
            .load()

        binding.pdfViewer.maxZoom = 6f
        return binding.root
    }

    override fun onSaveInstanceState(outState: Bundle) {
        outState.putInt(PDF_PAGE_KEY, binding.pdfViewer.currentPage)
        super.onSaveInstanceState(outState)
    }

    fun deleteLocalPdfInstance() {
        File(requireContext().cacheDir, OPENED_FILE_FOLDER + fileName).delete()
    }
}

const val PDF_PAGE_KEY = "pdf_page_key"
