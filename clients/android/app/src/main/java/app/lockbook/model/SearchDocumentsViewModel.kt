package app.lockbook.model

import android.app.Application
import android.graphics.Color
import android.graphics.Typeface
import android.text.Spannable
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
            CoreModel.startSearch(this@SearchDocumentsViewModel).unwrap()
        }
    }

    fun newSearch(query: String) {
        filesResults.clear()
        CoreModel.search(query).unwrap()
    }

    private fun endSearch() {
        CoreModel.endSearch().unwrap()
    }

    fun addFileNameSearchResult(id: String, path: String, score: Int, matchedIndicesJson: String) {
        val file = File(path)

        val parentPathSpan = (file.parentFile!!.path + "/").makeSpannableString()
        val fileNameSpan = file.name.makeSpannableString()

        val matchedIndices: List<Int> = Json.decodeFromString(matchedIndicesJson)

        for (index in matchedIndices) {
            if(index < parentPathSpan.length) {
                parentPathSpan.setSpan(BackgroundColorSpan(highlightColor), index, index + 1, Spannable.SPAN_INCLUSIVE_EXCLUSIVE)
            } else {
                val newIndex = index - parentPathSpan.length
                fileNameSpan.setSpan(BackgroundColorSpan(highlightColor), newIndex, newIndex + 1, Spannable.SPAN_INCLUSIVE_EXCLUSIVE)
            }
        }

        filesResults.add(SearchedDocumentViewHolderInfo.DocumentNameViewHolderInfo(id, parentPathSpan, fileNameSpan, score))
        filesResults.sortByDescending { it.score }

        viewModelScope.launch(Dispatchers.Main) {
            fileResultsSource.set(filesResults, { left, right -> left.id == right.id })
        }
    }

    fun addFileContentSearchResult(id: String, fileName: String, content: String, score: Int) {
//        Timber.e("FILE CONTENT ADDED: $id $fileName $score")
        viewModelScope.launch(Dispatchers.Main) {
//            fileResults.add(SearchedDocumentViewHolderInfo.DocumentContentViewHolderInfo(id, fileName, score, content))
        }
    }

    override fun onCleared() {
        endSearch()
    }
}

sealed class UpdateSearchUI {
    data class NewFileNameResult(val id: String, val score: Int, val name: String) : UpdateSearchUI()
    data class NewFileContentResult(val id: String, val score: Int, val file_name: String, val content: String) : UpdateSearchUI()
//    data class Error(val error: LbError) : UpdateSearchUI()
}