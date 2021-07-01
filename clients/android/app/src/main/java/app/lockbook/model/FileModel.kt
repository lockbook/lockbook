package app.lockbook.model

import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.preference.PreferenceManager
import app.lockbook.App
import app.lockbook.R
import app.lockbook.ui.BreadCrumbItem
import app.lockbook.util.*
import com.github.michaelbull.result.*
import timber.log.Timber

class FileModel(private val config: Config, private val _notifyError: SingleMutableLiveData<LbError>) {
    private val _files = MutableLiveData<List<ClientFileMetadata>>()
    private val _updateBreadcrumbBar = MutableLiveData<List<BreadCrumbItem>>()
    lateinit var parentFileMetadata: ClientFileMetadata
    lateinit var lastDocumentAccessed: ClientFileMetadata
    private val filePath: MutableList<ClientFileMetadata> = mutableListOf()

    val files: LiveData<List<ClientFileMetadata>>
        get() = _files

    val updateBreadcrumbBar: LiveData<List<BreadCrumbItem>>
        get() = _updateBreadcrumbBar

    fun isAtRoot(): Boolean = parentFileMetadata.id == parentFileMetadata.parent

    fun upADirectory() {
        when (
            val getSiblingsOfParentResult =
                CoreModel.getChildren(config, parentFileMetadata.parent)
        ) {
            is Ok -> {
                when (
                    val getParentOfParentResult =
                        CoreModel.getFileById(config, parentFileMetadata.parent)
                ) {
                    is Ok -> {
                        parentFileMetadata = getParentOfParentResult.value
                        if (filePath.size != 1) {
                            filePath.remove(filePath.last())
                        }
                        updateBreadCrumbWithLatest()
                        sortChildren(getSiblingsOfParentResult.value.filter { fileMetadata -> fileMetadata.id != fileMetadata.parent })
                    }
                    is Err -> _notifyError.postValue(getParentOfParentResult.error.toLbError())
                }
            }
            is Err -> _notifyError.postValue(getSiblingsOfParentResult.error.toLbError())
        }
    }

    fun intoFolder(fileMetadata: ClientFileMetadata) {
        parentFileMetadata = fileMetadata
        filePath.add(fileMetadata)
        refreshFiles()
    }

    fun startUpInRoot() {
        when (val getRootResult = CoreModel.getRoot(config)) {
            is Ok -> {
                parentFileMetadata = getRootResult.value
                filePath.add(getRootResult.value)
                updateBreadCrumbWithLatest()
                refreshFiles()
            }
            is Err -> _notifyError.postValue(getRootResult.error.toLbError())
        }
    }

    fun refreshFiles() {
        when (val getChildrenResult = CoreModel.getChildren(config, parentFileMetadata.id)) {
            is Ok -> {
                updateBreadCrumbWithLatest()
                sortChildren(getChildrenResult.value.filter { fileMetadata -> fileMetadata.id != fileMetadata.parent })
            }
            is Err -> _notifyError.postValue(getChildrenResult.error.toLbError())
        }
    }

    fun deleteFiles(ids: List<String>): Boolean {
        for (id in ids) {
            when(val result = CoreModel.deleteFile(config, id)) {
                is Ok -> {}
                is Err -> {
                    _notifyError.postValue(result.error.toLbError())
                    return false
                }
            }
        }
        return true
    }

    fun refreshAtParent(position: Int) {
        val firstChildPosition = position + 1
        for (index in firstChildPosition until filePath.size) {
            filePath.removeAt(firstChildPosition)
        }

        parentFileMetadata = filePath.last()
        refreshFiles()
    }

    private fun updateBreadCrumbWithLatest() {
        _updateBreadcrumbBar.postValue(filePath.map { file -> BreadCrumbItem(file.name) })
    }

    private fun sortFilesAlpha(files: List<ClientFileMetadata>, inReverse: Boolean): List<ClientFileMetadata> { // TODO: write less code by just reversing the original
        val sortAlpha = files.sortedBy { fileMetadata ->
            fileMetadata.name
        }

        return if(inReverse) sortAlpha.reversed() else sortAlpha
    }

    private fun sortFilesChanged(files: List<ClientFileMetadata>, inReverse: Boolean): List<ClientFileMetadata> {
        val sortChanged = files.sortedBy { fileMetadata ->
            fileMetadata.metadataVersion
        }

        return if(inReverse) sortChanged.reversed() else sortChanged
    }

    private fun sortFilesType(files: List<ClientFileMetadata>): List<ClientFileMetadata> {
        val tempFolders = files.filter { fileMetadata ->
            fileMetadata.fileType.name == FileType.Folder.name
        }
        val tempDocuments = files.filter { fileMetadata ->
            fileMetadata.fileType.name == FileType.Document.name
        }

        return tempFolders.union(
            tempDocuments.sortedWith(
                compareBy(
                    { fileMetadata ->
                        Regex(".[^.]+\$").find(fileMetadata.name)?.value
                    },
                    { fileMetaData ->
                        fileMetaData.name
                    }
                )
            )
        ).toList()
    }

    private fun sortChildren(files: List<ClientFileMetadata>) {
        val sortedFiles = when (
            val optionValue = PreferenceManager.getDefaultSharedPreferences(App.instance)
                .getString(SharedPreferences.SORT_FILES_KEY, SharedPreferences.SORT_FILES_A_Z)
        ) {
            SharedPreferences.SORT_FILES_A_Z -> sortFilesAlpha(files, false)
            SharedPreferences.SORT_FILES_Z_A -> sortFilesAlpha(files, true)
            SharedPreferences.SORT_FILES_LAST_CHANGED -> sortFilesChanged(files, false)
            SharedPreferences.SORT_FILES_FIRST_CHANGED -> sortFilesChanged(files, true)
            SharedPreferences.SORT_FILES_TYPE -> sortFilesType(files)
            else -> {
                Timber.e("File sorting shared preference does not match every supposed option: $optionValue")
                _notifyError.postValue(LbError.newProgError(basicErrorString()))
                return
            }
        }.exhaustive

        _files.postValue(sortedFiles)
    }
}
