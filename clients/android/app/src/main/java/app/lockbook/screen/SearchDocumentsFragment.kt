@file:Suppress("ktlint:standard:no-wildcard-imports")

package app.lockbook.screen

import android.annotation.SuppressLint
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.InputMethodManager
import androidx.activity.OnBackPressedCallback
import androidx.appcompat.widget.SearchView
import androidx.core.content.getSystemService
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
import com.google.android.material.color.MaterialColors
import java.lang.ref.WeakReference

class SearchDocumentsFragment : Fragment() {
    private lateinit var binding: FragmentSearchDocumentsBinding

    private val model: SearchDocumentsViewModel by viewModels()
    private val activityModel: StateViewModel by activityViewModels()

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    @SuppressLint("SetTextI18n")
    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?,
    ): View {
        binding = FragmentSearchDocumentsBinding.inflate(layoutInflater)
        model.setHighlightColors(
            MaterialColors.getColor(binding.root, com.google.android.material.R.attr.colorPrimaryContainer),
            MaterialColors.getColor(binding.root, com.google.android.material.R.attr.colorOnPrimaryContainer),
        )

        binding.searchDocumentsBack.setOnClickListener {
            showFiles()
        }

        requireActivity().onBackPressedDispatcher.addCallback(
            viewLifecycleOwner,
            object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    navigateBack()
                }
            },
        )

        model.updateSearchUI.observe(viewLifecycleOwner) { uiUpdate ->
            updateSearchUI(uiUpdate)
        }
        model.isProgressSpinnerShown.observe(viewLifecycleOwner) { isShown ->
            binding.searchDocumentsLoader.visibility = if (isShown) View.VISIBLE else View.GONE
        }
        model.isNoSearchResultsShown.observe(viewLifecycleOwner) { isShown ->
            binding.searchDocumentsNone.visibility = if (isShown) View.VISIBLE else View.GONE
        }

        binding.searchDocumentsResults.setup {
            withDataSource(model.fileResults)

            withItem<SearchedDocumentViewHolderInfo.SectionHeaderViewHolderInfo, SearchSectionHeaderViewHolder>(
                R.layout.search_section_header_item,
            ) {
                onBind(::SearchSectionHeaderViewHolder) { _, item ->
                    title.text = item.title
                    action.text = item.action
                    action.visibility = if (item.action == null) View.GONE else View.VISIBLE

                    action.setOnClickListener {
                        model.setFilenameSearchFocused(item.isFilenameSearchFocused)
                    }
                }
            }

            withItem<SearchedDocumentViewHolderInfo.EmptyViewHolderInfo, SearchEmptyViewHolder>(R.layout.search_empty_item) {
                onBind(::SearchEmptyViewHolder) { _, item ->
                    message.text = item.message
                }
            }

            withItem<SearchedDocumentViewHolderInfo.DocumentNameViewHolderInfo, SearchedDocumentNameViewHolder>(
                R.layout.searched_document_name_item,
            ) {
                onBind(::SearchedDocumentNameViewHolder) { _, item ->
                    icon.setImageResource(getDocumentIconResource(item.name.toString()))
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
                    icon.setImageResource(getDocumentIconResource(item.name.toString()))
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
                        model.setFocusedContentSearchResult(item.id)
                    }
                }

                onClick {
                    binding.searchDocumentsSearch.clearFocus()
                    activityModel.updateMainScreenUI(UpdateMainScreenUI.OpenFileFromSearch(item.id))
                }
            }
        }
        binding.searchDocumentsResults.addOnScrollListener(
            object : RecyclerView.OnScrollListener() {
                override fun onScrollStateChanged(
                    recyclerView: RecyclerView,
                    newState: Int,
                ) {
                    if (newState == RecyclerView.SCROLL_STATE_DRAGGING) {
                        dismissKeyboard()
                    }
                }
            },
        )

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
            is UpdateSearchUI.Error -> alertModel.notifyError(uiUpdate.error)
        }
    }

    private fun dismissKeyboard() {
        binding.searchDocumentsSearch.clearFocus()
        requireContext()
            .getSystemService<InputMethodManager>()
            ?.hideSoftInputFromWindow(binding.searchDocumentsSearch.windowToken, 0)
    }

    private fun navigateBack() {
        if (!model.navigateBackWithinSearch()) {
            showFiles()
        }
    }

    private fun showFiles() {
        activityModel.updateMainScreenUI(UpdateMainScreenUI.ShowFiles)
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
