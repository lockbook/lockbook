package app.lockbook.model

import android.app.Application
import android.os.Handler
import android.os.Looper
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import kotlinx.coroutines.*
import timber.log.Timber

class TextEditorViewModel(application: Application, val fileMetadata: File, private val text: String) :
    AndroidViewModel(application) {

    private val handler = Handler(Looper.myLooper()!!)
    var lastEdit = 0L
    val editHistory = EditTextModel.EditHistory()

    var currentContent: String = text

    private val _updateContent = MutableLiveData<Unit>()
    private val _notifyError = SingleMutableLiveData<LbError>()

    val notifyError: LiveData<LbError>
        get() = _notifyError

    val updateContent: LiveData<Unit>
        get() = _updateContent

    init {
        setUpTextView()
    }

    private fun setUpTextView() {
        _updateContent.postValue(Unit)
    }

    fun waitAndSaveContents(content: String) {
        currentContent = content
        editHistory.isDirty = true
        lastEdit = System.currentTimeMillis()
        val currentEdit = lastEdit

        Timber.e("SAVING")

        handler.postDelayed(
            {
                viewModelScope.launch(Dispatchers.IO) {
                    if (currentEdit == lastEdit && editHistory.isDirty) {
                        val writeToDocumentResult =
                            CoreModel.writeToDocument(fileMetadata.id, content)
                        if (writeToDocumentResult is Err) {
                            _notifyError.postValue(writeToDocumentResult.error.toLbError(getRes()))
                        } else {
                            editHistory.isDirty = false
                        }
                    }
                }
            },
            1000
        )
    }
}
