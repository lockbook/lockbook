@file:Suppress("ktlint:standard:no-wildcard-imports")

package app.lockbook.model

import android.app.Application
import android.text.Spannable
import android.text.SpannableString
import android.text.SpannableStringBuilder
import android.text.style.BackgroundColorSpan
import android.text.style.ForegroundColorSpan
import androidx.core.content.ContextCompat
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.R
import app.lockbook.util.SearchFocus
import app.lockbook.util.SearchedDocumentViewHolderInfo
import app.lockbook.util.SingleMutableLiveData
import app.lockbook.util.getContext
import app.lockbook.util.makeSpannableString
import com.afollestad.recyclical.datasource.emptyDataSourceTyped
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import net.lockbook.ContentSearcher
import net.lockbook.ContentSearcherResult
import net.lockbook.Lb
import net.lockbook.LbError
import net.lockbook.PathSearcher
import net.lockbook.PathSearcherResult
import net.lockbook.SearcherSnippet

class SearchDocumentsViewModel(
    application: Application,
) : AndroidViewModel(application) {
    private val _updateSearchUI = SingleMutableLiveData<UpdateSearchUI>()

    val updateSearchUI: LiveData<UpdateSearchUI>
        get() = _updateSearchUI

    val fileResults = emptyDataSourceTyped<SearchedDocumentViewHolderInfo>()

    var isProgressSpinnerShown = false
    var isNoSearchResultsShown = false

    private var pathSearcher: PathSearcher? = null
    private var contentSearcher: ContentSearcher? = null
    private var initJob: Job? = null
    private var searchJob: Job? = null
    private var input: String = ""
    private var focus: SearchFocus? = null
    private var focusedContentResultId: String? = null
    private var pathResults: Array<PathSearcherResult> = emptyArray()
    private var contentResults: Array<ContentSearcherResult> = emptyArray()
    private var suggestedResults: List<SearchedDocumentViewHolderInfo.DocumentNameViewHolderInfo> = emptyList()
    private val searchMutex = Mutex()

    private val overviewPreviewCount = 3
    private val contentSnippetContextChars = 40

    private val highlightBackgroundColor = ContextCompat.getColor(getContext(), R.color.md_theme_primaryContainer)
    private val highlightForegroundColor = ContextCompat.getColor(getContext(), R.color.md_theme_onPrimaryContainer)

    init {
        initJob = viewModelScope.launch(Dispatchers.IO) {
            try {
                searchMutex.withLock {
                    pathSearcher = pathSearcher ?: Lb.pathSearcher()
                    contentSearcher = contentSearcher ?: Lb.contentSearcher()
                }
                suggestedResults = loadSuggestedResults()
                withContext(Dispatchers.Main) {
                    if (input.isBlank()) {
                        renderInitialState()
                    }
                }
            } catch (err: LbError) {
                _updateSearchUI.postValue(UpdateSearchUI.Error(err))
            }
        }
    }

    fun newSearch(input: String) {
        this.input = input
        searchJob?.cancel()

        if (input.isBlank()) {
            hideProgressSpinnerIfVisible()
            hideNoSearchResultsIfVisible()
            renderInitialState()
            return
        }

        hideNoSearchResultsIfVisible()
        showProgressSpinnerIfGone()

        searchJob = viewModelScope.launch(Dispatchers.IO) {
            try {
                val pathSearcher = pathSearcher ?: Lb.pathSearcher().also { pathSearcher = it }
                val contentSearcher = contentSearcher ?: Lb.contentSearcher().also { contentSearcher = it }

                val (newPathResults, newContentResults) = searchMutex.withLock {
                    Pair(pathSearcher.query(input), contentSearcher.query(input))
                }

                if (!isActive) {
                    return@launch
                }

                pathResults = newPathResults
                contentResults = newContentResults

                val rows = buildSearchRows()
                withContext(Dispatchers.Main) {
                    hideProgressSpinnerIfVisible()
                    if (rows.isEmpty()) {
                        showNoSearchResultsIfGone()
                        fileResults.clear()
                    } else {
                        hideNoSearchResultsIfVisible()
                        fileResults.set(rows) { left, right -> left == right }
                    }
                }
            } catch (err: LbError) {
                _updateSearchUI.postValue(UpdateSearchUI.Error(err))
            } catch (err: IllegalStateException) {
                _updateSearchUI.postValue(UpdateSearchUI.Error(LbError().apply { msg = err.message ?: "Search is closed" }))
            }
        }
    }

    fun focusSearch(focus: SearchFocus?) {
        this.focus = focus
        focusedContentResultId = null
        renderCurrentState()
    }

    fun focusContentResult(id: String?) {
        focusedContentResultId = id
        focus = SearchFocus.Content
        renderCurrentState()
    }

    private fun renderCurrentState() {
        if (input.isBlank()) {
            renderInitialState()
        } else {
            fileResults.set(buildSearchRows()) { left, right -> left == right }
        }
    }

    private fun buildSearchRows(): List<SearchedDocumentViewHolderInfo> {
        val rows = mutableListOf<SearchedDocumentViewHolderInfo>()
        val focusedContentResult = focusedContentResultId?.let { id ->
            contentResults.firstOrNull { it.id == id }
        }

        if (focusedContentResult != null) {
            rows.add(
                SearchedDocumentViewHolderInfo.SectionHeaderViewHolderInfo(
                    "${focusedContentResult.filename} · ${focusedContentResult.matches.size} content matches",
                    "Back",
                    SearchFocus.Content
                )
            )
            rows.addAll(contentResultFocusedRows(focusedContentResult))
            return rows
        }

        when (focus) {
            SearchFocus.Filename -> {
                rows.add(SearchedDocumentViewHolderInfo.SectionHeaderViewHolderInfo("Filename matches", "Back"))
                rows.addAll(pathResults.map(::pathResultRow))
                if (pathResults.isEmpty()) {
                    rows.add(SearchedDocumentViewHolderInfo.EmptyViewHolderInfo("No filename matches"))
                }
            }
            SearchFocus.Content -> {
                rows.add(SearchedDocumentViewHolderInfo.SectionHeaderViewHolderInfo("Content matches", "Back"))
                rows.addAll(contentResults.mapNotNull(::contentResultRow))
                if (contentResults.isEmpty()) {
                    rows.add(SearchedDocumentViewHolderInfo.EmptyViewHolderInfo("No content matches"))
                }
            }
            null -> {
                rows.add(SearchedDocumentViewHolderInfo.SectionHeaderViewHolderInfo("Filename matches", expandableAction(pathResults.size), SearchFocus.Filename))
                rows.addAll(pathResults.take(overviewPreviewCount).map(::pathResultRow))
                if (pathResults.isEmpty()) {
                    rows.add(SearchedDocumentViewHolderInfo.EmptyViewHolderInfo("No filename matches"))
                }

                rows.add(SearchedDocumentViewHolderInfo.SectionHeaderViewHolderInfo("Content matches", expandableAction(contentResults.size), SearchFocus.Content))
                rows.addAll(contentResults.take(overviewPreviewCount).mapNotNull(::contentResultRow))
                if (contentResults.isEmpty()) {
                    rows.add(SearchedDocumentViewHolderInfo.EmptyViewHolderInfo("No content matches"))
                }
            }
        }

        return rows
    }

    private fun expandableAction(count: Int): String? =
        if (count > overviewPreviewCount) "Expand" else null

    private fun renderInitialState() {
        val rows = mutableListOf<SearchedDocumentViewHolderInfo>()

        if (suggestedResults.isNotEmpty()) {
            rows.add(SearchedDocumentViewHolderInfo.SectionHeaderViewHolderInfo("Suggested documents"))
            rows.addAll(suggestedResults)
        }

        fileResults.set(rows) { left, right -> left == right }
    }

    private fun loadSuggestedResults(): List<SearchedDocumentViewHolderInfo.DocumentNameViewHolderInfo> =
        Lb.suggestedDocs().mapNotNull { id ->
            runCatching {
                val file = Lb.getFileById(id)
                val parent = Lb.getFileById(file.parent)
                SearchedDocumentViewHolderInfo.DocumentNameViewHolderInfo(
                    file.id,
                    parent.name.makeSpannableString(),
                    file.name.makeSpannableString()
                )
            }.getOrNull()
        }

    private fun pathResultRow(result: PathSearcherResult): SearchedDocumentViewHolderInfo.DocumentNameViewHolderInfo {
        val (parentPathSpan, fileNameSpan) = highlightMatchedPathParts(result)
        return SearchedDocumentViewHolderInfo.DocumentNameViewHolderInfo(result.id, parentPathSpan, fileNameSpan)
    }

    private fun contentResultRow(result: ContentSearcherResult): SearchedDocumentViewHolderInfo.DocumentContentViewHolderInfo? {
        val snippets = result.matches
            .take(3)
            .mapNotNull { snippetForMatch(result, it) }
            .takeIf { it.isNotEmpty() }
            ?: return null

        return SearchedDocumentViewHolderInfo.DocumentContentViewHolderInfo(
            result.id,
            result.parentPath.makeSpannableString(),
            result.filename.makeSpannableString(),
            snippets,
            result.matches.size,
            result.matches.size > 3
        )
    }

    private fun contentResultFocusedRows(result: ContentSearcherResult): List<SearchedDocumentViewHolderInfo.DocumentContentViewHolderInfo> =
        result.matches.mapNotNull { match ->
            snippetForMatch(result, match)?.let { snippet ->
                SearchedDocumentViewHolderInfo.DocumentContentViewHolderInfo(
                    result.id,
                    result.parentPath.makeSpannableString(),
                    result.filename.makeSpannableString(),
                    listOf(snippet),
                    result.matches.size,
                    false
                )
            }
        }

    private fun snippetForMatch(result: ContentSearcherResult, match: net.lockbook.ContentSearcherMatch): SpannableString? {
        val snippet = contentSearcher?.snippet(result.id, match, contentSnippetContextChars) ?: return null
        return snippetSpannable(snippet)
    }

    private fun highlightMatchedPathParts(result: PathSearcherResult): Pair<SpannableString, SpannableString> {
        val parentPathSpan = result.parentPath.makeSpannableString()
        val fileNameSpan = result.filename.makeSpannableString()
        val parentOffset = if (result.parentPath == "/") 0 else 1
        val filenameOffset = if (result.parentPath.isEmpty() || result.parentPath == "/") 1 else result.parentPath.length + 2

        for (index in result.matchedIndices) {
            val parentIndex = index - parentOffset
            val filenameIndex = index - filenameOffset

            if (parentIndex >= 0 && parentIndex < parentPathSpan.length) {
                parentPathSpan.highlight(parentIndex, parentIndex + 1)
            } else if (filenameIndex >= 0 && filenameIndex < fileNameSpan.length) {
                fileNameSpan.highlight(filenameIndex, filenameIndex + 1)
            }
        }

        return Pair(parentPathSpan, fileNameSpan)
    }

    private fun snippetSpannable(snippet: SearcherSnippet): SpannableString {
        val builder = SpannableStringBuilder()
            .append(snippet.prefix)
            .append(snippet.matched)
            .append(snippet.suffix)
        val matchStart = snippet.prefix.length
        val matchEnd = matchStart + snippet.matched.length

        if (matchStart < matchEnd) {
            builder.highlight(matchStart, matchEnd)
        }

        return SpannableString(builder)
    }

    private fun Spannable.highlight(start: Int, end: Int) {
        setSpan(
            BackgroundColorSpan(highlightBackgroundColor),
            start,
            end,
            Spannable.SPAN_INCLUSIVE_EXCLUSIVE
        )
        setSpan(
            ForegroundColorSpan(highlightForegroundColor),
            start,
            end,
            Spannable.SPAN_INCLUSIVE_EXCLUSIVE
        )
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

    override fun onCleared() {
        initJob?.cancel()
        val job = searchJob
        if (job?.isActive == true) {
            job.invokeOnCompletion {
                closeSearchers()
            }
            job.cancel()
        } else {
            closeSearchers()
        }
    }

    private fun closeSearchers() {
        pathSearcher?.close()
        contentSearcher?.close()
        pathSearcher = null
        contentSearcher = null
    }
}

sealed class UpdateSearchUI {
    object ToggleNoSearchResults : UpdateSearchUI()

    object ToggleProgressSpinner : UpdateSearchUI()

    data class Error(
        val error: LbError,
    ) : UpdateSearchUI()
}
