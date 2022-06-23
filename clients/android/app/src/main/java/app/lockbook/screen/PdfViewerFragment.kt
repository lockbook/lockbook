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
import app.lockbook.model.AlertModel
import app.lockbook.model.DetailsScreen
import app.lockbook.model.OPENED_FILE_FOLDER
import app.lockbook.model.StateViewModel
import app.lockbook.util.Animate
import java.io.File
import java.lang.ref.WeakReference

class PdfViewerFragment : Fragment() {
    private var _binding: FragmentPdfViewerBinding? = null
    private val binding get() = _binding!!

    private var oldPage: Int? = null

    private val activityModel: StateViewModel by activityViewModels()
    private lateinit var fileName: String

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    private var isToolbarVisible = true
    private var isToolbarVisibleByClick = false

    companion object {
        const val PDF_PAGE_KEY = "pdf_page_key"
        const val IS_TOOLBAR_VISIBLE_BY_CLICK_KEY = "is_toolbar_visible_by_click_key"
        const val TOOLBAR_VISIBILITY_OFFSET = 0.01
        const val TOOLBAR_ALPHA = 100
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = FragmentPdfViewerBinding.inflate(inflater, container, false)
        val pdfViewerInfo = activityModel.detailsScreen as DetailsScreen.PdfViewer
        fileName = pdfViewerInfo.fileMetadata.decryptedName

        initializePdfRenderer(savedInstanceState, pdfViewerInfo)
        setUpAfterConfigurationChange(savedInstanceState)

        binding.pdfViewer.maxZoom = 6f
        return binding.root
    }

    private fun initializePdfRenderer(
        savedInstanceState: Bundle?,
        pdfViewerInfo: DetailsScreen.PdfViewer
    ) {
        oldPage = savedInstanceState?.getInt(PDF_PAGE_KEY)

        try {
            binding.pdfViewToolbar.title = fileName
            binding.pdfViewToolbar.setNavigationOnClickListener {
                activityModel.launchDetailsScreen(null)
            }
            binding.pdfViewToolbar.background.alpha = TOOLBAR_ALPHA
            binding.pdfViewer.fromFile(File(pdfViewerInfo.location, fileName))
                .enableDoubletap(true)
                .enableAnnotationRendering(true)
                .enableAntialiasing(true)
                .nightMode((resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES)
                .defaultPage(oldPage ?: 0)
                .onPageChange { page, pageCount ->
                    binding.pdfPageIndicator.text =
                        getString(R.string.pdf_page_indicator, page + 1, pageCount)
                }
                .onPageScroll { _, positionOffset ->
                    if (positionOffset < TOOLBAR_VISIBILITY_OFFSET && !isToolbarVisible) {
                        isToolbarVisible = true
                        Animate.animateVisibility(binding.pdfViewToolbar, View.VISIBLE, TOOLBAR_ALPHA, 200)
                    } else if (isToolbarVisible && !isToolbarVisibleByClick && positionOffset >= TOOLBAR_VISIBILITY_OFFSET) {
                        isToolbarVisible = false
                        Animate.animateVisibility(binding.pdfViewToolbar, View.GONE, 0, 200)
                    }
                }
                .onTap {
                    if (isToolbarVisible) {
                        isToolbarVisible = false
                        isToolbarVisibleByClick = false
                        Animate.animateVisibility(binding.pdfViewToolbar, View.GONE, 0, 200)
                    } else if (binding.pdfViewer.positionOffset >= TOOLBAR_VISIBILITY_OFFSET) {
                        isToolbarVisible = true
                        isToolbarVisibleByClick = true
                        Animate.animateVisibility(binding.pdfViewToolbar, View.VISIBLE, TOOLBAR_ALPHA, 200)
                    }

                    true
                }
                .onError {
                    alertModel.notify(getString(R.string.could_not_load_pdf)) {
                        activityModel.launchDetailsScreen(null)
                    }
                }
                .load()
        } catch (e: Exception) {
            alertModel.notify(getString(R.string.could_not_load_pdf)) {
                activityModel.launchDetailsScreen(null)
            }
        }
    }

    private fun setUpAfterConfigurationChange(savedInstanceState: Bundle?) {
        val maybeIsToolbarVisible = savedInstanceState?.getBoolean(IS_TOOLBAR_VISIBLE_KEY)
        val maybeIsToolbarVisibleByClick =
            savedInstanceState?.getBoolean(IS_TOOLBAR_VISIBLE_BY_CLICK_KEY)
        if (maybeIsToolbarVisible != null && maybeIsToolbarVisibleByClick != null) {
            isToolbarVisible = maybeIsToolbarVisible
            isToolbarVisibleByClick = maybeIsToolbarVisibleByClick

            if (!isToolbarVisible) {
                binding.pdfViewToolbar.visibility = View.GONE
            }
        }
    }

    override fun onSaveInstanceState(outState: Bundle) {
        outState.putInt(PDF_PAGE_KEY, binding.pdfViewer.currentPage)
        outState.putBoolean(IS_TOOLBAR_VISIBLE_KEY, isToolbarVisible)
        outState.putBoolean(IS_TOOLBAR_VISIBLE_BY_CLICK_KEY, isToolbarVisibleByClick)
        super.onSaveInstanceState(outState)
    }

    fun deleteLocalPdfInstance() {
        File(requireContext().cacheDir, OPENED_FILE_FOLDER + fileName).delete()
    }
}
