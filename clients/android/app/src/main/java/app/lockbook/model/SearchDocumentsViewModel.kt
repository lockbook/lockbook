package app.lockbook.model

import android.app.Application
import android.graphics.Color
import android.graphics.Typeface
import android.text.Spannable
import android.text.SpannableString
import android.text.SpannableStringBuilder
import android.text.style.BackgroundColorSpan
import android.text.style.ForegroundColorSpan
import android.text.style.StyleSpan
import androidx.core.content.res.ResourcesCompat
import androidx.core.text.toSpannable
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.R
import app.lockbook.util.*
import com.afollestad.recyclical.datasource.emptyDataSourceTyped
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.unwrap
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.Json
import timber.log.Timber
import java.io.File
import kotlin.coroutines.Continuation

class SearchDocumentsViewModel(application: Application) : AndroidViewModel(application) {

    private val _updateSearchUI = SingleMutableLiveData<UpdateSearchUI>()

    val updateSearchUI: LiveData<UpdateSearchUI>
        get() = _updateSearchUI

    private val filesResults = mutableListOf<SearchedDocumentViewHolderInfo>()
    val fileResultsSource = emptyDataSourceTyped<SearchedDocumentViewHolderInfo>()

    private val highlightColor = ResourcesCompat.getColor(getContext().resources, R.color.md_theme_inversePrimary, null)

    init {
        viewModelScope.launch(Dispatchers.IO) {
            val startSearchResult = CoreModel.startSearch(this@SearchDocumentsViewModel)

            if(startSearchResult is Err) {
                _updateSearchUI.value = UpdateSearchUI.Error(startSearchResult.error.toLbError(getRes()))
            }
        }
    }

    fun newSearch(query: String) {
        filesResults.clear()
        val searchResult = CoreModel.search(query)

        if(searchResult is Err) {
            _updateSearchUI.value = UpdateSearchUI.Error(searchResult.error.toLbError(getRes()))
        }
    }

    private fun endSearch() {
        val endSearchResult = CoreModel.endSearch()

        if(endSearchResult is Err) {
            _updateSearchUI.value = UpdateSearchUI.Error(endSearchResult.error.toLbError(getRes()))
        }
    }

    fun addFileNameSearchResult(id: String, path: String, score: Int, matchedIndicesJson: String) {
        val (parentPathSpan, fileNameSpan) = highlightMatchedPathParts(path, matchedIndicesJson)

        filesResults.add(SearchedDocumentViewHolderInfo.DocumentNameViewHolderInfo(id, parentPathSpan, fileNameSpan, score))
        filesResults.sortByDescending { it.score }

        viewModelScope.launch(Dispatchers.Main) {
            fileResultsSource.set(filesResults, { left, right -> left.id == right.id })
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

    fun addFileContentSearchResult(id: String, path: String, contentMatchesJson: String) {
        val (parentPath, fileName) = getPathAndParentFile(path)
        val contentMatches = highlightMatchedParagraph(contentMatchesJson)

        for(match in contentMatches) {
            Timber.e("THIS SECOND SCORE: ${match.second} ${fileName}")

            filesResults.add(SearchedDocumentViewHolderInfo.DocumentContentViewHolderInfo(id, parentPath, fileName, match.second, match.first))
        }

        filesResults.sortByDescending { it.score }

        viewModelScope.launch(Dispatchers.Main) {
            fileResultsSource.set(filesResults, { left, right -> left.id == right.id })
        }
    }

    private fun highlightMatchedParagraph(
        contentMatchesJson: String
    ): List<Pair<SpannableString, Int>> {
        val contentMatches: List<ContentMatch> = Json.decodeFromString(contentMatchesJson)
        val paragraphsSpan: MutableList<Pair<SpannableString, Int>> = mutableListOf()

        for(contentMatch in contentMatches) {
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
    data class OpenFile(val fileMetadata: DecryptedFileMetadata) : UpdateSearchUI()
    data class Error(val error: LbError) : UpdateSearchUI()
}