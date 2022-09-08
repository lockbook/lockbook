package app.lockbook.model

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.util.*
import com.afollestad.recyclical.datasource.emptyDataSourceTyped
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

class CreateLinkViewModel(application: Application) :
    AndroidViewModel(application) {
    lateinit var currentParent: File

    var files = emptyDataSourceTyped<File>()

    private val _closeFragment = MutableLiveData<Unit>()
    private val _updateTitle = MutableLiveData<String>()
    private val _notifyError = SingleMutableLiveData<LbError>()

    val closeFragment: LiveData<Unit>
        get() = _closeFragment

    val updateTitle: LiveData<String>
        get() = _updateTitle

    val notifyError: LiveData<LbError>
        get() = _notifyError

    init {
        viewModelScope.launch(Dispatchers.IO) {
            startAtRoot()
        }
    }

    private fun startAtRoot() {
        when (val getRootResult = CoreModel.getRoot()) {
            is Ok -> {
                currentParent = getRootResult.value
                refreshOverFolder()
            }
            is Err -> _notifyError.postValue(getRootResult.error.toLbError(getRes()))
        }.exhaustive
    }

    fun createLinkFile(name: String, id: String) {
        viewModelScope.launch(Dispatchers.IO) {
            val createLinkResult = CoreModel.createLink(name, id, currentParent.id)

            if (createLinkResult is Err) {
                _notifyError.postValue(createLinkResult.error.toLbError(getRes()))
                return@launch
            }

            _closeFragment.postValue(Unit)
        }
    }

    private fun refreshOverFolder() {
        when (val getChildrenResult = CoreModel.getChildren(currentParent.id)) {
            is Ok -> {
                _updateTitle.postValue(currentParent.name)
                val tempFiles = getChildrenResult.value.filter { file -> file.isFolder()}.toMutableList()

                viewModelScope.launch(Dispatchers.Main) {
                    files.set(FileModel.sortFiles(tempFiles))
                }
            }
            is Err -> _notifyError.postValue(getChildrenResult.error.toLbError(getRes()))
        }
    }

    fun refreshOverParent() {
        viewModelScope.launch(Dispatchers.IO) {
            if(currentParent.isRoot()) {
                _closeFragment.postValue(Unit)
            } else {
                when (val getFileById = CoreModel.getFileById(currentParent.parent)) {
                    is Ok -> {
                        currentParent = getFileById.value
                        refreshOverFolder()
                    }
                    is Err -> _notifyError.postValue(getFileById.error.toLbError(getRes()))
                }.exhaustive
            }
        }
    }

    fun onItemClick(item: File) {
        viewModelScope.launch(Dispatchers.IO) {
            currentParent = item
            refreshOverFolder()
        }
    }

}
