package app.lockbook.model

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.util.*
import com.afollestad.recyclical.datasource.emptyDataSourceTyped
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import net.lockbook.File
import net.lockbook.File.FileType
import net.lockbook.Lb
import net.lockbook.LbError

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
        try {
            currentParent = Lb.getRoot()
            refreshOverFolder()
        } catch (err: LbError) {
            _notifyError.postValue(err)
        }
    }

    private fun refreshOverFolder() {
        try {
            val children = Lb.getChildren(currentParent.id)
            _updateTitle.postValue(currentParent.name)
            val tempFiles = children.filter { file -> file.type == FileType.Folder }.toMutableList()

            viewModelScope.launch(Dispatchers.Main) {
                files.set(FileModel.sortFiles(tempFiles))
            }
        } catch (err: LbError) {
            _notifyError.postValue(err)
        }
    }

    fun refreshOverParent() {
        viewModelScope.launch(Dispatchers.IO) {
            if (currentParent.isRoot()) {
                _closeFragment.postValue(Unit)
            } else {
                try {
                    currentParent = Lb.getFileById(currentParent.parent)
                    refreshOverFolder()
                } catch (err: LbError) {
                    _notifyError.postValue(err)
                }
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
