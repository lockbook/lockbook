package app.lockbook.screen

import android.os.Bundle
import android.text.style.CharacterStyle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.TextView
import androidx.appcompat.widget.SearchView
import androidx.core.text.getSpans
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


class SearchDocumentsFragment: Fragment() {
    private lateinit var binding: FragmentSearchDocumentsBinding

    private val model: SearchDocumentsViewModel by viewModels()
    private val activityModel: StateViewModel by activityViewModels()

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
            }

            withItem<SearchedDocumentViewHolderInfo.DocumentContentViewHolderInfo, SearchedDocumentContentViewHolder>(R.layout.searched_document_name_item) {
                onBind(::SearchedDocumentContentViewHolder) { _, item ->
                    name.text = item.name
                    path.text = item.path
                    content.text = item.content
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
                if(newText != null && !newText.isEmpty())  {
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


    fun updateSearchUI(uiUpdate: UpdateSearchUI) {
        when(uiUpdate) {
            is UpdateSearchUI.NewFileContentResult -> {

            }
            is UpdateSearchUI.NewFileNameResult -> {

            }
        }
    }

}