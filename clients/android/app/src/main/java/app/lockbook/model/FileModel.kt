package app.lockbook.model

import android.content.Context
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.preference.PreferenceManager
import androidx.work.Worker
import androidx.work.WorkerParameters
import app.lockbook.App
import app.lockbook.ui.BreadCrumb
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import timber.log.Timber

class FileModel(path: String) {
    private val _files = MutableLiveData<List<FileMetadata>>()
    private val _updateBreadcrumbBar = SingleMutableLiveData<List<BreadCrumb>>()
    private val _errorHasOccurred = SingleMutableLiveData<String>()
    private val _unexpectedErrorHasOccurred = SingleMutableLiveData<String>()
    lateinit var parentFileMetadata: FileMetadata
    lateinit var lastDocumentAccessed: FileMetadata
    private val filePath: MutableList<FileMetadata> = mutableListOf()
    val config = Config(path)

    val files: LiveData<List<FileMetadata>>
        get() = _files

    val updateBreadcrumbBar: LiveData<List<BreadCrumb>>
        get() = _updateBreadcrumbBar

    val errorHasOccurred: LiveData<String>
        get() = _errorHasOccurred

    val unexpectedErrorHasOccurred: LiveData<String>
        get() = _unexpectedErrorHasOccurred

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
                        matchToDefaultSortOption(getSiblingsOfParentResult.value.filter { fileMetadata -> fileMetadata.id != fileMetadata.parent && !fileMetadata.deleted })
                    }
                    is Err -> when (val error = getParentOfParentResult.error) {
                        is GetFileByIdError.NoFileWithThatId -> _errorHasOccurred.postValue("Error! No file with that id!")
                        is GetFileByIdError.Unexpected -> {
                            Timber.e("Unable to get the parent of the current path: ${error.error}")
                            _unexpectedErrorHasOccurred.postValue(
                                error.error
                            )
                        }
                    }
                }
            }
            is Err -> when (val error = getSiblingsOfParentResult.error) {
                is GetChildrenError.Unexpected -> {
                    Timber.e("Unable to get siblings of the parent: ${error.error}")
                    _unexpectedErrorHasOccurred.postValue(error.error)
                }
            }
        }.exhaustive
    }

    fun updateBreadCrumbWithLatest() {
        _updateBreadcrumbBar.postValue(filePath.map { file -> BreadCrumb(file.name) })
    }

    fun intoFolder(fileMetadata: FileMetadata) {
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
            is Err -> when (val error = getRootResult.error) {
                is GetRootError.NoRoot -> _errorHasOccurred.postValue("No root!")
                is GetRootError.Unexpected -> {
                    Timber.e("Unable to set parent to root: ${error.error}")
                    _unexpectedErrorHasOccurred.postValue(
                        error.error
                    )
                }
            }
        }.exhaustive
    }

    fun refreshFiles() {
        when (val getChildrenResult = CoreModel.getChildren(config, parentFileMetadata.id)) {
            is Ok -> {
                updateBreadCrumbWithLatest()
                matchToDefaultSortOption(getChildrenResult.value.filter { fileMetadata -> fileMetadata.id != fileMetadata.parent && !fileMetadata.deleted })
            }
            is Err -> when (val error = getChildrenResult.error) {
                is GetChildrenError.Unexpected -> {
                    Timber.e("Unable to get children: ${getChildrenResult.error}")
                    _unexpectedErrorHasOccurred.postValue(error.error)
                }
            }
        }.exhaustive
    }

    fun deleteFiles(ids: List<String>): Boolean {
        for (id in ids) {
            if (!deleteFile(id)) {
                return false
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

    private fun deleteFile(id: String): Boolean {
        return when (val deleteFileResult = CoreModel.deleteFile(config, id)) {
            is Ok -> true
            is Err -> {
                when (val error = deleteFileResult.error) {
                    is FileDeleteError.FileDoesNotExist -> _errorHasOccurred.postValue("Error! The file you selected does not exist!")
                    is FileDeleteError.CannotDeleteRoot -> _errorHasOccurred.postValue("Error! The file you selected is root!")
                    is FileDeleteError.Unexpected -> {
                        Timber.e("Unable to delete file: ${error.error}")
                        _unexpectedErrorHasOccurred.postValue(
                            error.error
                        )
                    }
                }.exhaustive

                false
            }
        }.exhaustive
    }

    private fun sortFilesAlpha(files: List<FileMetadata>, inReverse: Boolean) {
        if (inReverse) {
            _files.postValue(
                files.sortedByDescending { fileMetadata ->
                    fileMetadata.name
                }
            )
        } else {
            _files.postValue(
                files.sortedBy { fileMetadata ->
                    fileMetadata.name
                }
            )
        }
    }

    private fun sortFilesChanged(files: List<FileMetadata>, inReverse: Boolean) {
        if (inReverse) {
            _files.postValue(
                files.sortedByDescending { fileMetadata ->
                    fileMetadata.metadataVersion
                }
            )
        } else {
            _files.postValue(
                files.sortedBy { fileMetadata ->
                    fileMetadata.metadataVersion
                }
            )
        }
    }

    private fun sortFilesType(files: List<FileMetadata>) {
        val tempFolders = files.filter { fileMetadata ->
            fileMetadata.fileType.name == FileType.Folder.name
        }
        val tempDocuments = files.filter { fileMetadata ->
            fileMetadata.fileType.name == FileType.Document.name
        }
        _files.postValue(
            tempFolders.union(
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
        )
    }

    private fun matchToDefaultSortOption(files: List<FileMetadata>) {
        when (
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
                _errorHasOccurred.postValue(BASIC_ERROR)
            }
        }.exhaustive
    }
}
