package app.lockbook.screen

import android.content.Context
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.InputMethodManager
import android.widget.TextView
import androidx.appcompat.widget.SearchView
import androidx.core.content.ContextCompat.getSystemService
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import app.lockbook.R
import app.lockbook.databinding.FragmentSearchDocumentsBinding
import app.lockbook.model.*
import app.lockbook.util.*
import com.afollestad.recyclical.setup
import com.afollestad.recyclical.withItem
import timber.log.Timber
import java.lang.ref.WeakReference


class SearchDocumentsFragment: Fragment() {
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
            withDataSource(model.fileResultsSource)
            withEmptyView(binding.searchDocumentsNone)

            withItem<SearchedDocumentViewHolderInfo.DocumentNameViewHolderInfo, SearchedDocumentNameViewHolder>(R.layout.searched_document_name_item) {
                onBind(::SearchedDocumentNameViewHolder) { _, item ->
                    name.setText(item.name, TextView.BufferType.SPANNABLE)
                    path.setText(item.path, TextView.BufferType.SPANNABLE)
                }

                onClick {
                    binding.searchDocumentsSearch.rootView.closeKeyboard()
                    model.openDocument(item.id)
                }
            }

            withItem<SearchedDocumentViewHolderInfo.DocumentContentViewHolderInfo, SearchedDocumentContentViewHolder>(R.layout.searched_document_content_item) {
                onBind(::SearchedDocumentContentViewHolder) { _, item ->
                    name.text = item.name
                    path.text = item.path
                    content.text = item.content
                }

                onClick {
                    binding.searchDocumentsSearch.closeKeyboard()
                    model.openDocument(item.id)
                }
            }
        }

        binding.searchDocumentsSearch.setOnQueryTextListener(object : SearchView.OnQueryTextListener {
            override fun onQueryTextSubmit(query: String?): Boolean {
                if(query != null && query.isNotEmpty())  {
                    model.newSearch(query)
                }

                Timber.e("Submit: $query")
                return true
            }

            override fun onQueryTextChange(newText: String?): Boolean {
                if(newText != null && newText.isNotEmpty())  {
                    model.newSearch(newText)
                } else if(newText?.isEmpty() == true) {
                    model.fileResultsSource.clear()
                }

                Timber.e("NEW Text: $newText")
                return true
            }
        })

        return binding.root
    }


    private fun updateSearchUI(uiUpdate: UpdateSearchUI) {
        when(uiUpdate) {
            is UpdateSearchUI.Error -> alertModel.notifyError(uiUpdate.error)
            is UpdateSearchUI.OpenFile -> activityModel.launchDetailsScreen(DetailsScreen.Loading(uiUpdate.fileMetadata))
        }
    }

}