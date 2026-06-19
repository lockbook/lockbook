@file:Suppress("ktlint:standard:no-wildcard-imports")

package app.lockbook.model

import android.app.Application
import android.text.Spannable
import android.text.SpannableString
import android.text.SpannableStringBuilder
import android.text.style.BackgroundColorSpan
import android.text.style.ForegroundColorSpan
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.util.SearchedDocumentViewHolderInfo
import app.lockbook.util.SingleMutableLiveData
import app.lockbook.util.makeSpannableString
import com.afollestad.recyclical.datasource.emptyDataSourceTyped
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExecutorCoroutineDispatcher
import kotlinx.coroutines.Job
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import net.lockbook.ContentSearcher
import net.lockbook.ContentSearcherMatch
import net.lockbook.ContentSearcherResult
import net.lockbook.Lb
import net.lockbook.LbError
import net.lockbook.PathSearcher
import net.lockbook.PathSearcherResult
import net.lockbook.SearcherSnippet
import java.util.concurrent.Executors

class SearchDocumentsViewModel(
    application: Application,
    private val filesListModel: FileTreeViewModel,
) : AndroidViewModel(application) {
    private val _updateSearchUI = SingleMutableLiveData<UpdateSearchUI>()

    val updateSearchUI: LiveData<UpdateSearchUI>
        get() = _updateSearchUI

    val fileResults = emptyDataSourceTyped<SearchedDocumentViewHolderInfo>()

    private val _isProgressSpinnerShown = MutableLiveData(false)
    val isProgressSpinnerShown: LiveData<Boolean>
        get() = _isProgressSpinnerShown

    private val _isNoSearchResultsShown = MutableLiveData(false)
    val isNoSearchResultsShown: LiveData<Boolean>
        get() = _isNoSearchResultsShown

    private var pathSearcher: PathSearcher? = null
    private var contentSearcher: ContentSearcher? = null
    private val searchDispatcher: ExecutorCoroutineDispatcher =
        Executors.newSingleThreadExecutor().asCoroutineDispatcher()
    private var searchJob: Job? = null
    private var progressSpinnerJob: Job? = null

    private var input: String = ""

    private var isFilenameSearchFocused = false
    private var focusedContentSearchResultId: String? = null

    private var highlightBackgroundColor: Int? = null
    private var highlightForegroundColor: Int? = null

    companion object {
        private const val OVERVIEW_PREVIEW_COUNT = 3
        private const val CONTENT_PREVIEW_SNIPPET_COUNT = 1
        private const val CONTENT_SNIPPET_CONTEXT_CHARS = 40
        private const val PROGRESS_SPINNER_DELAY_MS = 150L
    }

    init {
        // creates the searchers in the background and loads the suggested docs
        viewModelScope.launch(searchDispatcher) {
            try {
                pathSearcher = pathSearcher ?: Lb.pathSearcher()
                contentSearcher = contentSearcher ?: Lb.contentSearcher()
                val suggestedResults = loadSuggestedResults()
                withContext(Dispatchers.Main) {
                    if (input.isBlank()) {
                        renderInitialState(suggestedResults)
                    }
                }
            } catch (_: CancellationException) {
                // ViewModel is going away; no user-visible error.
            } catch (err: LbError) {
                _updateSearchUI.postValue(UpdateSearchUI.Error(err))
            }
        }
    }

    fun setHighlightColors(
        backgroundColor: Int,
        foregroundColor: Int,
    ) {
        highlightBackgroundColor = backgroundColor
        highlightForegroundColor = foregroundColor
    }

    fun newSearch(input: String) {
        this.input = input
        searchJob?.cancel()

        if (input.isBlank()) {
            hideProgressSpinner()
            hideNoSearchResults()
            renderCurrentState()
            return
        }

        hideNoSearchResults()
        scheduleProgressSpinner()

        searchJob =
            viewModelScope.launch(searchDispatcher) {
                try {
                    val (pathResults, contentResults) = querySearch(input)

                    if (!isActive) {
                        return@launch
                    }

                    val rows = buildSearchRows(pathResults, contentResults)
                    withContext(Dispatchers.Main) {
                        hideProgressSpinner()
                        if (rows.isEmpty()) {
                            showNoSearchResults()
                            fileResults.clear()
                        } else {
                            hideNoSearchResults()
                            fileResults.set(rows) { left, right -> left == right }
                        }
                    }
                } catch (_: CancellationException) {
                    // Expected when the user types another character or leaves search.
                } catch (err: LbError) {
                    if (isActive) {
                        _updateSearchUI.postValue(UpdateSearchUI.Error(err))
                    }
                } catch (err: IllegalStateException) {
                    if (isActive) {
                        _updateSearchUI.postValue(UpdateSearchUI.Error(LbError().apply { msg = err.message ?: "Search is closed" }))
                    }
                }
            }
    }

    fun setFilenameSearchFocused(focused: Boolean) {
        isFilenameSearchFocused = focused
        focusedContentSearchResultId = null
        renderCurrentState()
    }

    fun setFocusedContentSearchResult(id: String?) {
        focusedContentSearchResultId = id
        isFilenameSearchFocused = false
        renderCurrentState()
    }

    fun navigateBackWithinSearch(): Boolean {
        if (focusedContentSearchResultId != null) {
            focusedContentSearchResultId = null
            isFilenameSearchFocused = false
            renderCurrentState()
            return true
        }

        if (isFilenameSearchFocused) {
            isFilenameSearchFocused = false
            renderCurrentState()
            return true
        }

        return false
    }

    private fun renderCurrentState() {
        viewModelScope.launch(searchDispatcher) {
            if (input.isBlank()) {
                val suggestedResults = loadSuggestedResults()
                withContext(Dispatchers.Main) {
                    renderInitialState(suggestedResults)
                }
            } else {
                val (pathResults, contentResults) = querySearch(input)
                val rows = buildSearchRows(pathResults, contentResults)
                withContext(Dispatchers.Main) {
                    fileResults.set(rows) { left, right -> left == right }
                }
            }
        }
    }

    private fun querySearch(input: String): Pair<Array<PathSearcherResult>, Array<ContentSearcherResult>> {
        val pathSearcher = pathSearcher ?: Lb.pathSearcher().also { pathSearcher = it }
        val contentSearcher = contentSearcher ?: Lb.contentSearcher().also { contentSearcher = it }

        return Pair(pathSearcher.query(input), contentSearcher.query(input))
    }

    private fun buildSearchRows(
        pathResults: Array<PathSearcherResult>,
        contentResults: Array<ContentSearcherResult>,
    ): List<SearchedDocumentViewHolderInfo> {
        val rows = mutableListOf<SearchedDocumentViewHolderInfo>()
        val focusedContentSearchResult =
            focusedContentSearchResultId?.let { id ->
                contentResults.firstOrNull { it.id == id }
            }

        if (focusedContentSearchResult != null) {
            rows.add(
                SearchedDocumentViewHolderInfo.SectionHeaderViewHolderInfo(
                    "${focusedContentSearchResult.filename} · ${focusedContentSearchResult.matches.size} content matches",
                    "Back",
                ),
            )
            rows.addAll(focusedContentSearchResultRows(focusedContentSearchResult))
            return rows
        }

        if (isFilenameSearchFocused) {
            rows.add(SearchedDocumentViewHolderInfo.SectionHeaderViewHolderInfo("Filename matches", "Back"))
            val pathRows = pathResults.mapNotNull(::pathResultRow)
            rows.addAll(pathRows)
            if (pathRows.isEmpty()) {
                rows.add(SearchedDocumentViewHolderInfo.EmptyViewHolderInfo("No filename matches"))
            }
        } else {
            rows.add(
                SearchedDocumentViewHolderInfo.SectionHeaderViewHolderInfo(
                    "Filename matches",
                    if (pathResults.size > OVERVIEW_PREVIEW_COUNT) "Expand" else null,
                    true,
                ),
            )
            val pathRows = pathResults.take(OVERVIEW_PREVIEW_COUNT).mapNotNull(::pathResultRow)
            rows.addAll(pathRows)
            if (pathRows.isEmpty()) {
                rows.add(SearchedDocumentViewHolderInfo.EmptyViewHolderInfo("No filename matches"))
            }

            addContentRows(rows, contentResults)
        }

        return rows
    }

    private fun addContentRows(
        rows: MutableList<SearchedDocumentViewHolderInfo>,
        contentResults: Array<ContentSearcherResult>,
    ) {
        rows.add(SearchedDocumentViewHolderInfo.SectionHeaderViewHolderInfo("Content matches"))
        rows.addAll(contentResults.mapNotNull(::contentResultRow))
        if (contentResults.isEmpty()) {
            rows.add(SearchedDocumentViewHolderInfo.EmptyViewHolderInfo("No content matches"))
        }
    }

    private fun renderInitialState(suggestedResults: List<SearchedDocumentViewHolderInfo.DocumentNameViewHolderInfo>) {
        val rows = mutableListOf<SearchedDocumentViewHolderInfo>()

        if (suggestedResults.isNotEmpty()) {
            rows.add(SearchedDocumentViewHolderInfo.SectionHeaderViewHolderInfo("Suggested documents"))
            rows.addAll(suggestedResults)
        }

        fileResults.set(rows) { left, right -> left == right }
    }

    private fun loadSuggestedResults(): List<SearchedDocumentViewHolderInfo.DocumentNameViewHolderInfo> =
        Lb.suggestedDocs().mapNotNull { id ->
            val file = filesListModel.fileModel.idsAndFiles[id] ?: return@mapNotNull null
            val parent = filesListModel.fileModel.idsAndFiles[file.parent] ?: return@mapNotNull null
            SearchedDocumentViewHolderInfo.DocumentNameViewHolderInfo(
                file,
                parent.name.makeSpannableString(),
                file.name.makeSpannableString(),
            )
        }

    private fun pathResultRow(result: PathSearcherResult): SearchedDocumentViewHolderInfo.DocumentNameViewHolderInfo? {
        val (parentPathSpan, fileNameSpan) = result.toHighlightedPathParts()
        val file = filesListModel.fileModel.idsAndFiles[result.id] ?: return null
        return SearchedDocumentViewHolderInfo.DocumentNameViewHolderInfo(file, parentPathSpan, fileNameSpan)
    }

    private fun contentResultRow(result: ContentSearcherResult): SearchedDocumentViewHolderInfo.DocumentContentViewHolderInfo? {
        val snippets =
            result.matches
                .take(CONTENT_PREVIEW_SNIPPET_COUNT)
                .mapNotNull { it.toHighlightedSnippet(result) }
                .takeIf { it.isNotEmpty() }
                ?: return null
        val file = filesListModel.fileModel.idsAndFiles[result.id] ?: return null

        return SearchedDocumentViewHolderInfo.DocumentContentViewHolderInfo(
            file,
            result.parentPath.makeSpannableString(),
            result.filename.makeSpannableString(),
            snippets,
            result.matches.size,
            result.matches.size > CONTENT_PREVIEW_SNIPPET_COUNT,
        )
    }

    private fun focusedContentSearchResultRows(
        result: ContentSearcherResult,
    ): List<SearchedDocumentViewHolderInfo.DocumentContentViewHolderInfo> {
        val file = filesListModel.fileModel.idsAndFiles[result.id] ?: return emptyList()

        return result.matches.mapNotNull { match ->
            match.toHighlightedSnippet(result)?.let { snippet ->
                SearchedDocumentViewHolderInfo.DocumentContentViewHolderInfo(
                    file,
                    result.parentPath.makeSpannableString(),
                    result.filename.makeSpannableString(),
                    listOf(snippet),
                    result.matches.size,
                    false,
                )
            }
        }
    }

    private fun PathSearcherResult.toHighlightedPathParts(): Pair<SpannableString, SpannableString> {
        val parentPathSpan = parentPath.makeSpannableString()
        val fileNameSpan = filename.makeSpannableString()
        val parentOffset = if (parentPath == "/") 0 else 1
        val filenameOffset =
            if (parentPath.isEmpty() || parentPath == "/") {
                1
            } else {
                parentPath.codePointCount(0, parentPath.length) + 2
            }

        for (index in matchedIndices) {
            val parentIndex = index - parentOffset
            val filenameIndex = index - filenameOffset

            val parentRange = parentPath.spanRangeForCodePoint(parentIndex)
            val filenameRange = filename.spanRangeForCodePoint(filenameIndex)

            parentRange?.let { (start, end) ->
                parentPathSpan.highlight(start, end)
            }
            filenameRange?.let { (start, end) ->
                fileNameSpan.highlight(start, end)
            }
        }

        return Pair(parentPathSpan, fileNameSpan)
    }

    private fun String.spanRangeForCodePoint(index: Int): Pair<Int, Int>? {
        if (index < 0 || index >= codePointCount(0, length)) {
            return null
        }

        val start = offsetByCodePoints(0, index)
        val end = offsetByCodePoints(start, 1)
        return start to end
    }

    private fun ContentSearcherMatch.toHighlightedSnippet(result: ContentSearcherResult): SpannableString? {
        val snippet = contentSearcher?.snippet(result.id, this, CONTENT_SNIPPET_CONTEXT_CHARS) ?: return null
        return snippet.toHighlightedSpannable()
    }

    private fun SearcherSnippet.toHighlightedSpannable(): SpannableString {
        val builder =
            SpannableStringBuilder()
                .append(prefix)
                .append(matched)
                .append(suffix)
        val matchStart = prefix.length
        val matchEnd = matchStart + matched.length

        if (matchStart < matchEnd) {
            builder.highlight(matchStart, matchEnd)
        }

        return SpannableString(builder)
    }

    private fun Spannable.highlight(
        start: Int,
        end: Int,
    ) {
        val backgroundColor = highlightBackgroundColor ?: return
        val foregroundColor = highlightForegroundColor ?: return

        setSpan(
            BackgroundColorSpan(backgroundColor),
            start,
            end,
            Spannable.SPAN_INCLUSIVE_EXCLUSIVE,
        )
        setSpan(
            ForegroundColorSpan(foregroundColor),
            start,
            end,
            Spannable.SPAN_INCLUSIVE_EXCLUSIVE,
        )
    }

    private fun hideProgressSpinner() {
        progressSpinnerJob?.cancel()
        progressSpinnerJob = null
        _isProgressSpinnerShown.value = false
    }

    private fun showProgressSpinner() {
        _isProgressSpinnerShown.value = true
    }

    private fun hideNoSearchResults() {
        _isNoSearchResultsShown.value = false
    }

    private fun showNoSearchResults() {
        _isNoSearchResultsShown.value = true
    }

    private fun scheduleProgressSpinner() {
        progressSpinnerJob?.cancel()
        progressSpinnerJob =
            viewModelScope.launch(Dispatchers.Main) {
                delay(PROGRESS_SPINNER_DELAY_MS)
                showProgressSpinner()
            }
    }

    override fun onCleared() {
        runBlocking(searchDispatcher) {
            closeSearchers()
        }
        searchDispatcher.close()
        super.onCleared()
    }

    private fun closeSearchers() {
        pathSearcher?.close()
        contentSearcher?.close()
        pathSearcher = null
        contentSearcher = null
    }
}

sealed class UpdateSearchUI {
    data class Error(
        val error: LbError,
    ) : UpdateSearchUI()
}
