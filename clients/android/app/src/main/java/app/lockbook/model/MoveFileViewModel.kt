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

class MoveFileViewModel(application: Application, private val startId: String) :
    AndroidViewModel(application) {
    lateinit var currentParent: File
    lateinit var ids: List<String>

    var files = emptyDataSourceTyped<File>()

    private val _closeDialog = MutableLiveData<Unit>()
    private val _notifyError = SingleMutableLiveData<LbError>()

    val closeDialog: LiveData<Unit>
        get() = _closeDialog

    val notifyError: LiveData<LbError>
        get() = _notifyError

    companion object {
        const val PARENT_ID = "PARENT"
    }

    init {
        viewModelScope.launch(Dispatchers.IO) {
            startWithCurrentParent()
        }
    }

    private fun startWithCurrentParent() {
        try {
            currentParent = Lb.getFileById(startId)
            refreshOverFolder()
        } catch (err: LbError) {
            _notifyError.postValue(err)
        }
    }

    fun moveFilesToCurrentFolder() {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                for (id in ids) {
                    Lb.moveFile(id, currentParent.id)
                }
            } catch (err: LbError) {
                _notifyError.postValue(err)
                return@launch
            }

            _closeDialog.postValue(Unit)
        }
    }

    fun refreshOverFolder() {
        try {
            val tempFiles = Lb.getChildren(currentParent.id).filter { file ->
                file.type == FileType.Folder && !ids.contains(file.id)
            }.toMutableList()

            if (!currentParent.isRoot) {
                val parent = File()
                parent.id = PARENT_ID
                parent.type = FileType.Folder
                parent.name = "..."
                tempFiles.add(0, parent)
            }

            viewModelScope.launch(Dispatchers.Main) {
                files.set(FileModel.sortFiles(tempFiles))
            }
        } catch (err: LbError) {
            _notifyError.postValue(err)
        }
    }

    private fun setParentAsParent() {
        try {
            currentParent = Lb.getFileById(currentParent.parent)
        } catch (err: LbError) {
            _notifyError.postValue(err)
        }
    }

    fun onItemClick(item: File) {
        viewModelScope.launch(Dispatchers.IO) {
            when (item.id) {
                PARENT_ID -> {
                    setParentAsParent()
                    refreshOverFolder()
                }
                else -> {
                    currentParent = item
                    refreshOverFolder()
                }
            }
        }
    }
}
