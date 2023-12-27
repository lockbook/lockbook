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
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.Json
import java.io.File

class SearchDocumentsViewModel(application: Application) : AndroidViewModel(application) {

    private val _updateSearchUI = SingleMutableLiveData<UpdateSearchUI>()

    val updateSearchUI: LiveData<UpdateSearchUI>
        get() = _updateSearchUI

    private val filesResultsSource = mutableListOf<SearchedDocumentViewHolderInfo>()
    val fileResults = emptyDataSourceTyped<SearchedDocumentViewHolderInfo>()

    var isProgressSpinnerShown = false
    var isNoSearchResultsShown = false

    var lastQuery = ""

    private val highlightColor = ResourcesCompat.getColor(getContext().resources, R.color.md_theme_inversePrimary, null)

    init {
        viewModelScope.launch(Dispatchers.IO) {
            val startSearchResult = CoreModel.startSearch(this@SearchDocumentsViewModel)

            if (startSearchResult is Err) {
                _updateSearchUI.postValue(UpdateSearchUI.Error(startSearchResult.error.toLbError(getRes())))
            }
        }
    }

    fun newSearch(query: String?) {
        hideNoSearchResultsIfVisible()

        if (query == null) {
            return
        }

        lastQuery = query

        showProgressSpinnerIfGone()

        viewModelScope.launch(Dispatchers.IO) {
            val searchResult = CoreModel.search(query)

            if (searchResult is Err) {
                _updateSearchUI.value = UpdateSearchUI.Error(searchResult.error.toLbError(getRes()))
            }
        }
    }

    private fun endSearch() {
        val endSearchResult = CoreModel.endSearch()

        if (endSearchResult is Err) {
            _updateSearchUI.value = UpdateSearchUI.Error(endSearchResult.error.toLbError(getRes()))
        }
    }

    // used by core over ffi
    fun startOfSearchQuery() {
        viewModelScope.launch(Dispatchers.Main) {
            filesResultsSource.clear()
            fileResults.clear()

            showProgressSpinnerIfGone()
            hideNoSearchResultsIfVisible()
        }
    }

    // used by core over ffi
    fun addFileNameSearchResult(id: String, path: String, score: Int, matchedIndicesJson: String) {
        val (parentPathSpan, fileNameSpan) = highlightMatchedPathParts(path, matchedIndicesJson)

        filesResultsSource.add(SearchedDocumentViewHolderInfo.DocumentNameViewHolderInfo(id, parentPathSpan, fileNameSpan, score))
        filesResultsSource.sortByDescending { it.score }

        viewModelScope.launch(Dispatchers.Main) {
            fileResults.set(filesResultsSource, { left, right -> left.id == right.id })
        }
    }

    // used by core over ffi
    fun addFileContentSearchResult(id: String, path: String, contentMatchesJson: String) {
        val (parentPath, fileName) = getPathAndParentFile(path)
        val contentMatches = highlightMatchedParagraph(contentMatchesJson)

        for (match in contentMatches) {
            filesResultsSource.add(SearchedDocumentViewHolderInfo.DocumentContentViewHolderInfo(id, parentPath, fileName, match.second, match.first))
        }

        filesResultsSource.sortByDescending { it.score }

        viewModelScope.launch(Dispatchers.Main) {
            fileResults.set(filesResultsSource, { left, right -> left.id == right.id })
        }
    }

    // used by core over ffi
    fun endOfSearchQuery() {
        viewModelScope.launch(Dispatchers.Main) {
            hideProgressSpinnerIfVisible()

            if (fileResults.isEmpty() && lastQuery.isNotEmpty()) {
                showNoSearchResultsIfGone()
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
        matchedIndicesJson: String
    ): Pair<SpannableString, SpannableString> {
        val (parentPathSpan, fileNameSpan) = getPathAndParentFile(path)

        val matchedIndices: List<Int> = Json.decodeFromString(matchedIndicesJson)

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
        contentMatchesJson: String
    ): List<Pair<SpannableString, Int>> {
        val contentMatches: List<ContentMatch> = Json.decodeFromString(contentMatchesJson)
        val paragraphsSpan: MutableList<Pair<SpannableString, Int>> = mutableListOf()

        for (contentMatch in contentMatches) {
            val paragraphSpan = contentMatch.paragraph.makeSpannableString()

            paragraphsSpan.add(Pair(paragraphSpan, contentMatch.score))

            for (index in contentMatch.matchedIndices) {
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

    fun openDocument(id: String) {
        viewModelScope.launch(Dispatchers.IO) {
            val updateSearchUI = when (val result = CoreModel.getFileById(id)) {
                is Ok -> UpdateSearchUI.OpenFile(result.value)
                is Err -> UpdateSearchUI.Error(result.error.toLbError(getRes()))
            }

            _updateSearchUI.postValue(updateSearchUI)
        }
    }

    override fun onCleared() {
        endSearch()
    }
}

sealed class UpdateSearchUI {
    object ToggleNoSearchResults : UpdateSearchUI()
    object ToggleProgressSpinner : UpdateSearchUI()
    data class OpenFile(val fileMetadata: app.lockbook.util.File) : UpdateSearchUI()
    data class Error(val error: LbError) : UpdateSearchUI()
}
