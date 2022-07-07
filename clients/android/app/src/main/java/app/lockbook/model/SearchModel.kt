package app.lockbook.model

import app.lockbook.util.SingleMutableLiveData
import kotlinx.coroutines.channels.Channel

class SearchModel() {

    private val _updateSearchUI = SingleMutableLiveData<UpdateSearchUI>()

    val searchQuery: Channel<String?> = Channel()

    suspend fun getSearchQuery(): String? = searchQuery.receive()


    fun addFileNameSearchResult(id: String, name: String, score: Int) {

    }

    fun addFileContentSearchResult(id: String, fileName: String, content: String, score: Int) {

    }


}

sealed class UpdateSearchUI {
    data class NewFileNameResult(val id: String, val score: Int, val name: String) : UpdateSearchUI()
    data class NewFileContentResult(val id: String, val score: Int, val file_name: String, val content: String) : UpdateSearchUI()
}

