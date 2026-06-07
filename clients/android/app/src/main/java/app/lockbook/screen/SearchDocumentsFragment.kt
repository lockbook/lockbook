@file:Suppress("ktlint:standard:no-wildcard-imports")

package app.lockbook.screen

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.InputMethodManager
import androidx.appcompat.widget.SearchView
import androidx.core.content.getSystemService
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import androidx.recyclerview.widget.RecyclerView
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
        savedInstanceState: Bundle?,
    ): View {
        binding = FragmentSearchDocumentsBinding.inflate(layoutInflater)
        ViewCompat.setOnApplyWindowInsetsListener(binding.root) { _, insets ->
            val statusBarTop = insets.getInsets(WindowInsetsCompat.Type.statusBars()).top
            binding.searchDocumentsStatusBarBackground.layoutParams =
                binding.searchDocumentsStatusBarBackground.layoutParams.apply {
                    height = statusBarTop
                }
            insets
        }

        binding.searchDocumentsToolbar.setNavigationOnClickListener {
            activityModel.updateMainScreenUI(UpdateMainScreenUI.ShowFiles)
        }

        model.updateSearchUI.observe(viewLifecycleOwner) { uiUpdate ->
            updateSearchUI(uiUpdate)
        }

        binding.searchDocumentsResults.setup {
            withDataSource(model.fileResults)

            withItem<SearchedDocumentViewHolderInfo.SectionHeaderViewHolderInfo, SearchSectionHeaderViewHolder>(R.layout.search_section_header_item) {
                onBind(::SearchSectionHeaderViewHolder) { _, item ->
                    title.text = item.title
                    action.text = item.action
                    action.visibility = if (item.action == null) View.GONE else View.VISIBLE

                    action.setOnClickListener {
                        model.focusSearch(item.focus)
                    }
                }
            }

            withItem<SearchedDocumentViewHolderInfo.EmptyViewHolderInfo, SearchEmptyViewHolder>(R.layout.search_empty_item) {
                onBind(::SearchEmptyViewHolder) { _, item ->
                    message.text = item.message
                }
            }

            withItem<SearchedDocumentViewHolderInfo.DocumentNameViewHolderInfo, SearchedDocumentNameViewHolder>(R.layout.searched_document_name_item) {
                onBind(::SearchedDocumentNameViewHolder) { _, item ->
                    name.text = item.name
                    path.text = item.path
                }

                onClick {
                    binding.searchDocumentsSearch.clearFocus()
                    activityModel.updateMainScreenUI(UpdateMainScreenUI.OpenFileFromSearch(item.id))
                }
            }

            withItem<SearchedDocumentViewHolderInfo.DocumentContentViewHolderInfo, SearchedDocumentContentViewHolder>(
                R.layout.searched_document_content_item,
            ) {
                onBind(::SearchedDocumentContentViewHolder) { _, item ->
                    name.text = item.name
                    path.text = item.path
                    val snippetViews = listOf(content1, content2, content3)
                    snippetViews.forEachIndexed { index, view ->
                        val snippet = item.contents.getOrNull(index)
                        view.text = snippet
                        view.visibility = if (snippet == null) View.GONE else View.VISIBLE
                    }

                    showMore.text = "Show more (${item.totalMatches})"
                    showMore.visibility = if (item.showMore) View.VISIBLE else View.GONE
                    showMore.setOnClickListener {
                        model.focusContentResult(item.id)
                    }
                }

                onClick {
                    binding.searchDocumentsSearch.clearFocus()
                    activityModel.updateMainScreenUI(UpdateMainScreenUI.OpenFileFromSearch(item.id))
                }
            }
        }
        binding.searchDocumentsResults.addOnScrollListener(object : RecyclerView.OnScrollListener() {
            override fun onScrollStateChanged(recyclerView: RecyclerView, newState: Int) {
                if (newState == RecyclerView.SCROLL_STATE_DRAGGING) {
                    dismissKeyboard()
                }
            }
        })

        binding.searchDocumentsSearch.setOnQueryTextFocusChangeListener { _, focus ->
            if (focus) {
                requireActivity().window.requestKeyboardFocus(binding.searchDocumentsSearch.findFocus())
            }
        }

        binding.searchDocumentsSearch.setOnQueryTextListener(
            object : SearchView.OnQueryTextListener {
                override fun onQueryTextSubmit(query: String?): Boolean {
                    model.newSearch(query ?: "")
                    binding.searchDocumentsSearch.clearFocus()

                    return true
                }

                override fun onQueryTextChange(newText: String?): Boolean {
                    model.newSearch(newText ?: "")

                    return true
                }
            },
        )

        binding.searchDocumentsSearch.requestFocus()
        return binding.root
    }

    private fun updateSearchUI(uiUpdate: UpdateSearchUI) {
        when (uiUpdate) {
            UpdateSearchUI.ToggleProgressSpinner -> {
                binding.searchDocumentsLoader.visibility =
                    if (model.isProgressSpinnerShown) View.VISIBLE else View.GONE
            }

            UpdateSearchUI.ToggleNoSearchResults -> {
                binding.searchDocumentsNone.visibility =
                    if (model.isNoSearchResultsShown) View.VISIBLE else View.GONE
            }

            is UpdateSearchUI.Error -> {
                alertModel.notifyError(uiUpdate.error)
            }

            else -> {}
        }
    }

    private fun dismissKeyboard() {
        binding.searchDocumentsSearch.clearFocus()
        requireContext()
            .getSystemService<InputMethodManager>()
            ?.hideSoftInputFromWindow(binding.searchDocumentsSearch.windowToken, 0)
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
