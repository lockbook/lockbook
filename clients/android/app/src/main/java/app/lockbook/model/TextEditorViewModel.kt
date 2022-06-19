package app.lockbook.model

import android.app.Application
import android.os.Handler
import android.os.Looper
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.util.getRes
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import kotlinx.coroutines.*

class TextEditorViewModel(application: Application, val id: String, private val text: String) :
    AndroidViewModel(application) {

    private val handler = Handler(Looper.myLooper()!!)
    var lastEdit = 0L

    val editHistory = EditTextModel.EditHistory()

    private val _content = SingleMutableLiveData<String>()
    private val _notifyError = SingleMutableLiveData<LbError>()

    val notifyError: LiveData<LbError>
        get() = _notifyError

    val content: MutableLiveData<String>
        get() = _content

    init {
        setUpTextView()
    }

    private fun setUpTextView() {
        _content.postValue(text)
    }

    fun waitAndSaveContents(content: String) {
        editHistory.isDirty = true
        lastEdit = System.currentTimeMillis()
        val currentEdit = lastEdit

        handler.postDelayed(
            {
                viewModelScope.launch(Dispatchers.IO) {
                    if (currentEdit == lastEdit && editHistory.isDirty) {
                        val writeToDocumentResult =
                            CoreModel.writeToDocument(id, content)
                        if (writeToDocumentResult is Err) {
                            _notifyError.postValue(writeToDocumentResult.error.toLbError(getRes()))
                        } else {
                            editHistory.isDirty = false
                        }
                    }
                }
            },
            5000
        )
    }
}
