package app.lockbook.model

import android.app.Application
import android.os.Handler
import android.os.Looper
import android.text.Editable
import android.text.TextWatcher
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.App.Companion.config
import app.lockbook.getRes
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.*
import timber.log.Timber

class TextEditorViewModel(application: Application, private val id: String) :
    AndroidViewModel(application) {

    private val handler = Handler(Looper.myLooper()!!)
    private var lastEdit = 0L

    private val _content = MutableLiveData<String>()
    private val _notifyError = MutableLiveData<LbError>()

    val notifyError: LiveData<LbError>
        get() = _notifyError

    val content: MutableLiveData<String>
        get() = _content

    init {
        setUpTextView()
    }

    private fun setUpTextView() {
        viewModelScope.launch(Dispatchers.IO) {
            when (val documentResult = CoreModel.readDocument(config, id)) {
                is Ok -> _content.postValue(documentResult.value)
                is Err -> _notifyError.postValue(documentResult.error.toLbError(getRes()))
            }
        }
    }

    fun waitAndSaveContents(content: String) {
        viewModelScope.launch(Dispatchers.IO) {
            lastEdit = System.currentTimeMillis()
            val currentEdit = lastEdit

            handler.postDelayed({
                if(currentEdit == lastEdit) {
                    saveContents(content)
                }
            }, 5000)
        }
    }

    fun saveContents(content: String) {
        val writeToDocumentResult = CoreModel.writeToDocument(config, id, content)
        if (writeToDocumentResult is Err) {
            _notifyError.postValue(writeToDocumentResult.error.toLbError(getRes()))
        }
    }
}
