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

class TextEditorViewModel(application: Application, val fileMetadata: File, private val text: String) :
    AndroidViewModel(application) {

    private val handler = Handler(Looper.myLooper()!!)
    var lastEdit = 0L
    var isDirty = false
    var currentContent = text

    var savedCursorStart = -1
    var savedCursorEnd = -1

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
}
