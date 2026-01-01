package app.lockbook.screen

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.appcompat.widget.SearchView
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import app.lockbook.R
import app.lockbook.databinding.FragmentSearchDocumentsBinding
import app.lockbook.model.*
import app.lockbook.util.*
import com.afollestad.recyclical.setup
import com.afollestad.recyclical.withItem
import java.lang.ref.WeakReference

class SearchDocumentsFragment : Fragment() {
    private lateinit var binding: FragmentSearchDocumentsBinding

    private val model: SearchDocumentsViewModel by viewModels()
    private val activityModel: StateViewModel by activityViewModels()

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        binding = FragmentSearchDocumentsBinding.inflate(layoutInflater)


        binding.searchDocumentsToolbar.setNavigationOnClickListener {
            activityModel.updateMainScreenUI(UpdateMainScreenUI.ShowFiles)
        }

        model.updateSearchUI.observe(viewLifecycleOwner) { uiUpdate ->
            updateSearchUI(uiUpdate)
        }

        binding.searchDocumentsResults.setup {
            withDataSource(model.fileResults)

            withItem<SearchedDocumentViewHolderInfo.DocumentNameViewHolderInfo, SearchedDocumentNameViewHolder>(R.layout.searched_document_name_item) {
                onBind(::SearchedDocumentNameViewHolder) { _, item ->
                    name.text = item.name
                    path.text = item.path
                }

                onClick {
                    binding.searchDocumentsSearch.clearFocus()
                    activityModel.updateMainScreenUI(UpdateMainScreenUI.OpenFile(item.id))
                }
            }

            withItem<SearchedDocumentViewHolderInfo.DocumentContentViewHolderInfo, SearchedDocumentContentViewHolder>(R.layout.searched_document_content_item) {
                onBind(::SearchedDocumentContentViewHolder) { _, item ->
                    name.text = item.name
                    path.text = item.path
                    content.text = item.content
                }

                onClick {
                    binding.searchDocumentsSearch.clearFocus()
                    activityModel.updateMainScreenUI(UpdateMainScreenUI.OpenFile(item.id))
                }
            }
        }

        binding.searchDocumentsSearch.setOnQueryTextFocusChangeListener { _, focus ->
            if (focus) {
                requireActivity().window.requestKeyboardFocus(binding.searchDocumentsSearch.findFocus())
            }
        }

        binding.searchDocumentsSearch.setOnQueryTextListener(object : SearchView.OnQueryTextListener {
            override fun onQueryTextSubmit(query: String?): Boolean {
                model.newSearch(query ?: "")
                binding.searchDocumentsSearch.clearFocus()

                return true
            }

            override fun onQueryTextChange(newText: String?): Boolean {
                model.newSearch(newText ?: "")

                return true
            }
        })

        binding.searchDocumentsSearch.requestFocus()
        return binding.root
    }

    private fun updateSearchUI(uiUpdate: UpdateSearchUI) {
        when (uiUpdate) {
            UpdateSearchUI.ToggleProgressSpinner -> binding.searchDocumentsLoader.visibility = if (model.isProgressSpinnerShown) View.VISIBLE else View.GONE
            UpdateSearchUI.ToggleNoSearchResults -> binding.searchDocumentsNone.visibility = if (model.isNoSearchResultsShown) View.VISIBLE else View.GONE
            is UpdateSearchUI.Error -> alertModel.notifyError(uiUpdate.error)
            else -> {}
        }
    }

    override fun onResume() {
        activityModel.updateMainScreenUI(UpdateMainScreenUI.ToggleBottomViewNavigation)
        super.onResume()
    }

    override fun onStop() {
        activityModel.updateMainScreenUI(UpdateMainScreenUI.ToggleBottomViewNavigation)
        super.onStop()
    }

}
