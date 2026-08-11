@file:Suppress("ktlint:standard:no-wildcard-imports")

package app.lockbook.screen

import android.annotation.SuppressLint
import android.os.Bundle
import android.text.TextWatcher
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.EditorInfo
import androidx.activity.BackEventCompat
import androidx.activity.OnBackPressedCallback
import androidx.core.widget.doAfterTextChanged
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.databinding.FragmentSearchDocumentsBinding
import app.lockbook.model.*
import app.lockbook.util.*
import com.afollestad.recyclical.setup
import com.afollestad.recyclical.withItem
import com.google.android.material.color.MaterialColors
import com.google.android.material.listitem.ListItemLayout
import com.google.android.material.search.SearchView
import com.google.android.material.transition.MaterialFadeThrough
import net.lockbook.File
import net.lockbook.File.FileType
import java.lang.ref.WeakReference

class SearchDocumentsFragment : Fragment() {
    private lateinit var binding: FragmentSearchDocumentsBinding
    private var searchTextWatcher: TextWatcher? = null

    val presentation: SearchPresentation
        get() =
            arguments
                ?.getString(PRESENTATION_KEY)
                ?.let { value -> SearchPresentation.entries.firstOrNull { it.name == value } }
                ?: SearchPresentation.FullScreen

    private val isSidebarMorph: Boolean
        get() = presentation == SearchPresentation.SidebarMorph

    private val searchView
        get() = binding.searchDocumentsSearchView

    companion object {
        private const val PRESENTATION_KEY = "presentation"

        fun newInstance(presentation: SearchPresentation): SearchDocumentsFragment =
            SearchDocumentsFragment().apply {
                arguments = Bundle().apply { putString(PRESENTATION_KEY, presentation.name) }
            }
    }

    private val model: SearchDocumentsViewModel by viewModels(
        factoryProducer = {
            object : ViewModelProvider.Factory {
                @Suppress("UNCHECKED_CAST")
                override fun <T : ViewModel> create(modelClass: Class<T>): T {
                    if (modelClass.isAssignableFrom(SearchDocumentsViewModel::class.java)) {
                        return SearchDocumentsViewModel(
                            requireActivity().application,
                            ViewModelProvider(requireActivity())[FileTreeViewModel::class.java],
                        ) as T
                    }
                    throw IllegalArgumentException("Unknown ViewModel class")
                }
            }
        },
    )
    private val mainScreenModel: MainScreenViewModel by activityViewModels()
    private val fileTreeModel: FileTreeViewModel by activityViewModels()

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        if (!isSidebarMorph) {
            enterTransition = MaterialFadeThrough()
            exitTransition = MaterialFadeThrough()
        }
    }

    @SuppressLint("SetTextI18n")
    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?,
    ): View {
        binding = FragmentSearchDocumentsBinding.inflate(inflater, container, false)
        if (isSidebarMorph) {
            val activity = requireActivity() as MainScreenActivity
            searchView.setupWithSearchBar(activity.binding.sidebarSearchBar)
            // setupWithSearchBar installs its own click listener. Keep opening search
            // routed through navigation state instead of directly showing this view.
            activity.configureSidebarSearchLauncher()
        } else {
            searchView.setVisible(true)
        }
        model.setHighlightColors(
            MaterialColors.getColor(binding.root, com.google.android.material.R.attr.colorPrimaryContainer),
            MaterialColors.getColor(binding.root, com.google.android.material.R.attr.colorOnPrimaryContainer),
        )

        requireActivity().onBackPressedDispatcher.addCallback(
            viewLifecycleOwner,
            object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    navigateBack()
                }

                override fun handleOnBackStarted(backEvent: BackEventCompat) {
                    if (isSidebarMorph && !model.canNavigateBackWithinSearch()) {
                        searchView.startBackProgress(backEvent)
                    }
                }

                override fun handleOnBackProgressed(backEvent: BackEventCompat) {
                    if (isSidebarMorph && !model.canNavigateBackWithinSearch()) {
                        searchView.updateBackProgress(backEvent)
                    }
                }

                override fun handleOnBackCancelled() {
                    if (isSidebarMorph && !model.canNavigateBackWithinSearch()) {
                        searchView.cancelBackProgress()
                    }
                }
            },
        )

        if (isSidebarMorph) {
            searchView.addTransitionListener { _, _, newState ->
                if (newState == SearchView.TransitionState.HIDDEN) {
                    showFiles()
                }
            }
        }

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

            withItem<SearchedDocumentViewHolderInfo.DocumentNameViewHolderInfo, FileMetadataViewHolder>(
                R.layout.file_metadata_item,
            ) {
                onBind(::FileMetadataViewHolder) { index, item ->
                    updateSearchResultAppearance(itemView, index)
                    bind(
                        FileMetadataRowInfo(
                            file = item.file,
                            title = item.name,
                            subtitle = item.path,
                            iconRes = item.file.getIconResource(),
                            background = FileMetadataRowBackground.SurfaceContainerHigh,
                        ),
                    )

                    fileItemHolder.setOnClickListener {
                        openSearchResult(item.file)
                    }
                }
            }

            withItem<SearchedDocumentViewHolderInfo.DocumentContentViewHolderInfo, SearchedDocumentContentViewHolder>(
                R.layout.searched_document_content_item,
            ) {
                onBind(::SearchedDocumentContentViewHolder) { index, item ->
                    updateSearchResultAppearance(itemView, index)
                    icon.setImageResource(item.file.getIconResource())
                    name.text = item.name
                    path.text = item.path
                    val snippet = item.contents.getOrNull(0)
                    content.text = snippet
                    content.visibility = if (snippet == null) View.GONE else View.VISIBLE

                    showMore.text = "Show more (${item.totalMatches})"
                    showMore.visibility = if (item.showMore) View.VISIBLE else View.GONE
                    showMore.setOnClickListener {
                        model.setFocusedContentSearchResult(item.file.id)
                    }

                    itemHolder.setOnClickListener {
                        openSearchResult(item.file)
                    }
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

        searchView.toolbar.setNavigationOnClickListener {
            navigateBack()
        }
        searchView.toolbar.menu.clear()
        searchView.editText.imeOptions = EditorInfo.IME_ACTION_SEARCH or EditorInfo.IME_FLAG_NO_EXTRACT_UI
        searchTextWatcher =
            searchView.editText.doAfterTextChanged { text ->
                model.newSearch(text?.toString().orEmpty())
            }
        searchView.editText.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == EditorInfo.IME_ACTION_SEARCH) {
                model.newSearch(searchView.text.toString())
                searchView.clearFocusAndHideKeyboard()
                true
            } else {
                false
            }
        }
        if (isSidebarMorph) {
            searchView.post(searchView::show)
        } else {
            searchView.post(searchView::requestFocusAndShowKeyboard)
        }
        return binding.root
    }

    private fun openSearchResult(file: File) {
        searchView.clearFocusAndHideKeyboard()
        when (file.type) {
            FileType.Document -> {
                mainScreenModel.navigate(MainNavigationAction.OpenDocument(file.id, newFile = false))
            }

            FileType.Folder -> {
                fileTreeModel.enterFolder(file)
                mainScreenModel.navigate(MainNavigationAction.OpenFolderFromSearch)
            }

            FileType.Link -> {} // shouldn't happen
        }
    }

    private fun updateSearchResultAppearance(
        itemView: View,
        index: Int,
    ) {
        val rows = model.fileResults.toList()
        var sectionStart = index
        var sectionEnd = index

        while (sectionStart > 0 && rows[sectionStart - 1].isSearchResultRow()) {
            sectionStart--
        }

        while (sectionEnd < rows.lastIndex && rows[sectionEnd + 1].isSearchResultRow()) {
            sectionEnd++
        }

        (itemView as? ListItemLayout)?.updateAppearance(index - sectionStart, sectionEnd - sectionStart + 1)
    }

    private fun SearchedDocumentViewHolderInfo.isSearchResultRow(): Boolean =
        this is SearchedDocumentViewHolderInfo.DocumentNameViewHolderInfo ||
            this is SearchedDocumentViewHolderInfo.DocumentContentViewHolderInfo

    private fun updateSearchUI(uiUpdate: UpdateSearchUI) {
        when (uiUpdate) {
            is UpdateSearchUI.Error -> alertModel.notifyError(uiUpdate.error)
        }
    }

    private fun dismissKeyboard() {
        searchView.clearFocusAndHideKeyboard()
    }

    private fun navigateBack() {
        if (!model.navigateBackWithinSearch()) {
            if (isSidebarMorph) {
                searchView.handleBackInvoked()
            } else {
                showFiles()
            }
        }
    }

    private fun showFiles() {
        mainScreenModel.navigate(MainNavigationAction.CloseSearch)
    }

    override fun onDestroyView() {
        searchTextWatcher?.let(searchView.editText::removeTextChangedListener)
        searchTextWatcher = null
        searchView.editText.setOnEditorActionListener(null)
        binding.searchDocumentsResults.adapter = null
        if (isSidebarMorph) {
            searchView.setupWithSearchBar(null)
            (activity as? MainScreenActivity)?.configureSidebarSearchLauncher()
        }
        super.onDestroyView()
    }
}
