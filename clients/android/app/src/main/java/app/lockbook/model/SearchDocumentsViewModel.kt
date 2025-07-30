package app.lockbook.model

import android.app.Application
import android.text.Spannable
import android.text.SpannableString
import android.text.style.BackgroundColorSpan
import androidx.core.content.res.ResourcesCompat
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.R
import app.lockbook.util.*
import com.afollestad.recyclical.datasource.emptyDataSourceTyped
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import net.lockbook.Lb
import net.lockbook.LbError
import net.lockbook.SearchResult
import net.lockbook.SearchResult.DocumentMatch.ContentMatch
import java.io.File
import java.util.Arrays

class SearchDocumentsViewModel(application: Application) : AndroidViewModel(application) {

    private val _updateSearchUI = SingleMutableLiveData<UpdateSearchUI>()

    val updateSearchUI: LiveData<UpdateSearchUI>
        get() = _updateSearchUI

    val fileResults = emptyDataSourceTyped<SearchedDocumentViewHolderInfo>()

    var isProgressSpinnerShown = false
    var isNoSearchResultsShown = false

    private val highlightColor = ResourcesCompat.getColor(getContext().resources, R.color.md_theme_inversePrimary, null)

    init {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                processSearchResults(Lb.search(""))
            } catch (err: LbError) {
                _updateSearchUI.postValue(UpdateSearchUI.Error(err))
            }
        }
    }

    private fun processSearchResults(results: Array<SearchResult>) {
        viewModelScope.launch(Dispatchers.Main) {
            hideProgressSpinnerIfVisible()
        }

        val filesResultsSource = mutableListOf<SearchedDocumentViewHolderInfo>()

        for (result in results) {
            when (result) {
                is SearchResult.PathMatch -> {
                    val (parentPathSpan, fileNameSpan) = highlightMatchedPathParts(result.path, result.matchedIndices)

                    filesResultsSource.add(SearchedDocumentViewHolderInfo.DocumentNameViewHolderInfo(result.id, parentPathSpan, fileNameSpan, result.score))
                }
                is SearchResult.DocumentMatch -> {
                    val (parentPath, fileName) = getPathAndParentFile(result.path)
                    val contentMatches = highlightMatchedParagraph(result.contentMatches)

                    for (contentMatch in contentMatches) {
                        filesResultsSource.add(SearchedDocumentViewHolderInfo.DocumentContentViewHolderInfo(result.id, parentPath, fileName, contentMatch.second, contentMatch.first))
                    }
                }
            }
        }

        viewModelScope.launch(Dispatchers.Main) {
            if (filesResultsSource.isEmpty()) {
                showNoSearchResultsIfGone()
            } else {
                fileResults.set(filesResultsSource, { left, right -> left.id == right.id })
            }
        }
    }

    fun newSearch(input: String) {
        hideNoSearchResultsIfVisible()
        showProgressSpinnerIfGone()
        fileResults.clear()

        viewModelScope.launch(Dispatchers.IO) {
            try {
                processSearchResults(Lb.search(input))
            } catch (err: LbError) {
                _updateSearchUI.postValue(UpdateSearchUI.Error(err))
            }
        }
    }

    private fun hideProgressSpinnerIfVisible() {
        if (isProgressSpinnerShown) {
            isProgressSpinnerShown = false
            _updateSearchUI.value = UpdateSearchUI.ToggleProgressSpinner
        }
    }

    private fun hideNoSearchResultsIfVisible() {
        if (isNoSearchResultsShown) {
            isNoSearchResultsShown = false
            _updateSearchUI.value = UpdateSearchUI.ToggleNoSearchResults
        }
    }

    private fun showProgressSpinnerIfGone() {
        if (!isProgressSpinnerShown) {
            isProgressSpinnerShown = true
            _updateSearchUI.value = UpdateSearchUI.ToggleProgressSpinner
        }
    }

    private fun showNoSearchResultsIfGone() {
        if (!isNoSearchResultsShown) {
            isNoSearchResultsShown = true
            _updateSearchUI.value = UpdateSearchUI.ToggleNoSearchResults
        }
    }

    private fun highlightMatchedPathParts(
        path: String,
        matchedIndices: IntArray
    ): Pair<SpannableString, SpannableString> {
        val (parentPathSpan, fileNameSpan) = getPathAndParentFile(path)

        for (index in matchedIndices) {
            if (index < parentPathSpan.length) {
                parentPathSpan.setSpan(
                    BackgroundColorSpan(highlightColor),
                    index,
                    index + 1,
                    Spannable.SPAN_INCLUSIVE_EXCLUSIVE
                )
            } else {
                val newIndex = index - parentPathSpan.length
                fileNameSpan.setSpan(
                    BackgroundColorSpan(highlightColor),
                    newIndex,
                    newIndex + 1,
                    Spannable.SPAN_INCLUSIVE_EXCLUSIVE
                )
            }
        }
        return Pair(parentPathSpan, fileNameSpan)
    }

    private fun highlightMatchedParagraph(
        contentMatches: Array<ContentMatch>
    ): List<Pair<SpannableString, Int>> {
        val paragraphsSpan: MutableList<Pair<SpannableString, Int>> = mutableListOf()

        for (contentMatch in contentMatches) {
            val paragraphSpan = contentMatch.paragraph.makeSpannableString()

            paragraphsSpan.add(Pair(paragraphSpan, contentMatch.score))
            for (index in contentMatch.matchedIndices) {
                // avoid index out of bounds error
                if (index >= contentMatch.paragraph.length){
                    break
                }

                paragraphSpan.setSpan(
                    BackgroundColorSpan(highlightColor),
                    index,
                    index + 1,
                    Spannable.SPAN_INCLUSIVE_EXCLUSIVE
                )
            }
        }

        return paragraphsSpan
    }

    private fun getPathAndParentFile(path: String): Pair<SpannableString, SpannableString> {
        val file = File(path)
        return Pair((file.parentFile!!.path + "/").makeSpannableString(), (file.name).makeSpannableString())
    }

    override fun onCleared() {
    }
}

sealed class UpdateSearchUI {
    object ToggleNoSearchResults : UpdateSearchUI()
    object ToggleProgressSpinner : UpdateSearchUI()
    data class Error(val error: LbError) : UpdateSearchUI()
}
