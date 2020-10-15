package app.lockbook.loggedin.listfiles

import android.content.Context
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.preference.PreferenceManager
import androidx.work.Worker
import androidx.work.WorkerParameters
import app.lockbook.App
import app.lockbook.utils.*
import app.lockbook.utils.Messages.UNEXPECTED_CLIENT_ERROR
import app.lockbook.utils.Messages.UNEXPECTED_ERROR
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.Result
import timber.log.Timber

class FileModel(path: String) {
    private val _files = MutableLiveData<List<FileMetadata>>()
    private val _errorHasOccurred = SingleMutableLiveData<String>()
    private val _unexpectedErrorHasOccurred = SingleMutableLiveData<String>()
    private lateinit var parentFileMetadata: FileMetadata
    private lateinit var lastDocumentAccessed: FileMetadata
    val config = Config(path)

    val files: LiveData<List<FileMetadata>>
        get() = _files

    val errorHasOccurred: LiveData<String>
        get() = _errorHasOccurred

    val unexpectedErrorHasOccurred: LiveData<String>
        get() = _unexpectedErrorHasOccurred

    fun syncWorkAvailable(): Boolean {
        when (val syncWorkResult = CoreModel.calculateFileSyncWork(config)) {
            is Ok -> return true
            is Err -> when (val error = syncWorkResult.error) {
                is CalculateWorkError.NoAccount -> {
                    Timber.e("No account.")
                    _errorHasOccurred.postValue("Error! No account!")
                }
                is CalculateWorkError.CouldNotReachServer -> {
                    Timber.e("Could not reach server despite being online.")
                    _errorHasOccurred.postValue(
                        UNEXPECTED_CLIENT_ERROR
                    )
                }
                is CalculateWorkError.UnexpectedError -> {
                    Timber.e("Unable to calculate syncWork: ${error.error}")
                    _unexpectedErrorHasOccurred.postValue(
                        UNEXPECTED_ERROR
                    )
                }
            }
        }

        return false
    }

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
                        matchToDefaultSortOption(getSiblingsOfParentResult.value.filter { fileMetadata -> fileMetadata.id != fileMetadata.parent && !fileMetadata.deleted })
                    }
                    is Err -> when (val error = getParentOfParentResult.error) {
                        is GetFileByIdError.NoFileWithThatId -> _errorHasOccurred.postValue("Error! No file with that id!")
                        is GetFileByIdError.UnexpectedError -> {
                            Timber.e("Unable to get the parent of the current path: ${error.error}")
                            _unexpectedErrorHasOccurred.postValue(
                                UNEXPECTED_ERROR
                            )
                        }
                        else -> {
                            Timber.e("GetFileByIdError not matched: ${error::class.simpleName}.")
                            _errorHasOccurred.postValue(UNEXPECTED_CLIENT_ERROR)
                        }
                    }
                }
            }
            is Err -> when (val error = getSiblingsOfParentResult.error) {
                is GetChildrenError.UnexpectedError -> {
                    Timber.e("Unable to get siblings of the parent: ${error.error}")
                    _unexpectedErrorHasOccurred.postValue(UNEXPECTED_ERROR)
                }
                else -> {
                    Timber.e("GetChildrenError not matched: ${error::class.simpleName}.")
                    _errorHasOccurred.postValue(UNEXPECTED_CLIENT_ERROR)
                }
            }
        }
    }

    fun renameRefreshFiles(id: String, newName: String) {
        when (val renameFileResult = CoreModel.renameFile(config, id, newName)) {
            is Ok -> refreshFiles()
            is Err -> when (val error = renameFileResult.error) {
                is RenameFileError.FileDoesNotExist -> _errorHasOccurred.postValue("Error! File does not exist!")
                is RenameFileError.NewNameContainsSlash -> _errorHasOccurred.postValue("Error! New name contains slash!")
                is RenameFileError.FileNameNotAvailable -> _errorHasOccurred.postValue("Error! File name not available!")
                is RenameFileError.NewNameEmpty -> _errorHasOccurred.postValue("Error! New file name cannot be empty!")
                is RenameFileError.CannotRenameRoot -> _errorHasOccurred.postValue("Error! Cannot rename root!")
                is RenameFileError.UnexpectedError -> {
                    Timber.e("Unable to rename file: ${error.error}")
                    _unexpectedErrorHasOccurred.postValue(
                        UNEXPECTED_ERROR
                    )
                }
                else -> {
                    Timber.e("RenameFileError not matched: ${error::class.simpleName}.")
                    _errorHasOccurred.postValue(UNEXPECTED_CLIENT_ERROR)
                }
            }
        }
    }

    fun deleteRefreshFiles(id: String) {
        when (val deleteFileResult = CoreModel.deleteFile(config, id)) {
            is Ok -> refreshFiles()
            is Err -> when (val error = deleteFileResult.error) {
                is DeleteFileError.NoFileWithThatId -> _errorHasOccurred.postValue("Error! No file with that id!")
                is DeleteFileError.UnexpectedError -> {
                    Timber.e("Unable to delete file: ${error.error}")
                    _unexpectedErrorHasOccurred.postValue(
                        UNEXPECTED_ERROR
                    )
                }
                else -> {
                    Timber.e("DeleteFileError not matched: ${error::class.simpleName}.")
                    _errorHasOccurred.postValue(UNEXPECTED_CLIENT_ERROR)
                }
            }
        }
    }

    fun handleReadDocument(fileMetadata: FileMetadata): EditableFile? {
        when (val documentResult = CoreModel.getDocumentContent(config, fileMetadata.id)) {
            is Ok -> {
                lastDocumentAccessed = fileMetadata
                return EditableFile(fileMetadata.name, fileMetadata.id, documentResult.value.secret)
            }
            is Err -> when (val error = documentResult.error) {
                is ReadDocumentError.TreatedFolderAsDocument -> _errorHasOccurred.postValue("Error! Folder treated as document!")
                is ReadDocumentError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                is ReadDocumentError.FileDoesNotExist -> _errorHasOccurred.postValue("Error! File does not exist!")
                is ReadDocumentError.UnexpectedError -> {
                    Timber.e("Unable to get content of file: ${error.error}")
                    _unexpectedErrorHasOccurred.postValue(
                        UNEXPECTED_ERROR
                    )
                }
                else -> {
                    Timber.e("ReadDocumentError not matched: ${error::class.simpleName}.")
                    _errorHasOccurred.postValue(UNEXPECTED_CLIENT_ERROR)
                }
            }
        }

        return null
    }

    fun intoFolder(fileMetadata: FileMetadata) {
        parentFileMetadata = fileMetadata
        refreshFiles()
    }

    fun startUpInRoot() {
        when (val getRootResult = CoreModel.getRoot(config)) {
            is Ok -> {
                parentFileMetadata = getRootResult.value
                refreshFiles()
            }
            is Err -> when (val error = getRootResult.error) {
                is GetRootError.NoRoot -> _errorHasOccurred.postValue("No root!")
                is GetRootError.UnexpectedError -> {
                    Timber.e("Unable to set parent to root: ${error.error}")
                    _unexpectedErrorHasOccurred.postValue(
                        UNEXPECTED_ERROR
                    )
                }
                else -> {
                    Timber.e("GetRootError not matched: ${error::class.simpleName}.")
                    _errorHasOccurred.postValue(UNEXPECTED_CLIENT_ERROR)
                }
            }
        }
    }

    fun createInsertRefreshFiles(name: String, fileType: String) {
        when (
            val createFileResult =
                CoreModel.createFile(config, parentFileMetadata.id, name, fileType)
        ) {
            is Ok -> {
                val insertFileResult = CoreModel.insertFile(config, createFileResult.value)
                if (insertFileResult is Err) {
                    when (val error = insertFileResult.error) {
                        is InsertFileError.UnexpectedError -> {
                            Timber.e("Unable to insert a newly created file: ${insertFileResult.error}")
                            _unexpectedErrorHasOccurred.postValue(UNEXPECTED_ERROR)
                        }
                        else -> {
                            Timber.e("InsertFileError not matched: ${error::class.simpleName}.")
                            _errorHasOccurred.postValue(UNEXPECTED_CLIENT_ERROR)
                        }
                    }
                }

                refreshFiles()
            }
            is Err -> when (val error = createFileResult.error) {
                is CreateFileError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                is CreateFileError.DocumentTreatedAsFolder -> _errorHasOccurred.postValue("Error! Document is treated as folder!")
                is CreateFileError.CouldNotFindAParent -> _errorHasOccurred.postValue("Error! Could not find file parent!")
                is CreateFileError.FileNameNotAvailable -> _errorHasOccurred.postValue("Error! File name not available!")
                is CreateFileError.FileNameContainsSlash -> _errorHasOccurred.postValue("Error! File contains a slash!")
                is CreateFileError.FileNameEmpty -> _errorHasOccurred.postValue("Error! File cannot be empty!")
                is CreateFileError.UnexpectedError -> {
                    Timber.e("Unable to create a file: ${error.error}")
                    _unexpectedErrorHasOccurred.postValue(
                        UNEXPECTED_ERROR
                    )
                }
                else -> {
                    Timber.e("CreateFileError not matched: ${error::class.simpleName}.")
                    _errorHasOccurred.postValue(UNEXPECTED_CLIENT_ERROR)
                }
            }
        }
    }

    fun refreshFiles() {
        when (val getChildrenResult = CoreModel.getChildren(config, parentFileMetadata.id)) {
            is Ok -> {
                matchToDefaultSortOption(getChildrenResult.value.filter { fileMetadata -> fileMetadata.id != fileMetadata.parent && !fileMetadata.deleted })
            }
            is Err -> when (val error = getChildrenResult.error) {
                is GetChildrenError.UnexpectedError -> {
                    Timber.e("Unable to get children: ${getChildrenResult.error}")
                    _unexpectedErrorHasOccurred.postValue(UNEXPECTED_ERROR)
                }
                else -> {
                    Timber.e("GetChildrenError not matched: ${error::class.simpleName}.")
                    _errorHasOccurred.postValue(UNEXPECTED_CLIENT_ERROR)
                }
            }
        }
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
                    fileMetadata.metadata_version
                }
            )
        } else {
            _files.postValue(
                files.sortedBy { fileMetadata ->
                    fileMetadata.metadata_version
                }
            )
        }
    }

    private fun sortFilesType(files: List<FileMetadata>) {
        val tempFolders = files.filter { fileMetadata ->
            fileMetadata.file_type.name == FileType.Folder.name
        }
        val tempDocuments = files.filter { fileMetadata ->
            fileMetadata.file_type.name == FileType.Document.name
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

    fun matchToDefaultSortOption(files: List<FileMetadata>) {
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
                _errorHasOccurred.postValue(UNEXPECTED_CLIENT_ERROR)
            }
        }
    }

    fun determineSizeOfSyncWork(): Result<Int, CalculateWorkError> {
        when (val syncWorkResult = CoreModel.calculateFileSyncWork(config)) {
            is Ok -> return Ok(syncWorkResult.value.work_units.size)
            is Err -> {
                when (val error = syncWorkResult.error) {
                    is CalculateWorkError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                    is CalculateWorkError.CouldNotReachServer -> Timber.e("Could not reach server.")
                    is CalculateWorkError.UnexpectedError -> {
                        Timber.e("Unable to calculate syncWork: ${error.error}")
                        _unexpectedErrorHasOccurred.postValue(
                            UNEXPECTED_ERROR
                        )
                    }
                }

                return syncWorkResult
            }
        }
    }

    class SyncWork(appContext: Context, workerParams: WorkerParameters) :
        Worker(appContext, workerParams) {
        override fun doWork(): Result {
            val syncAllResult =
                CoreModel.syncAllFiles(Config(applicationContext.filesDir.absolutePath))
            return if (syncAllResult is Err) {
                when (val error = syncAllResult.error) {
                    is SyncAllError.NoAccount -> {
                        Timber.e("No account.")
                        Result.failure()
                    }
                    is SyncAllError.CouldNotReachServer -> {
                        Timber.e("Could not reach server.")
                        Result.retry()
                    }
                    is SyncAllError.ExecuteWorkError -> {
                        Timber.e("Could not execute some work: ${Klaxon().toJsonString(error.error)}")
                        Result.failure()
                    }
                    is SyncAllError.UnexpectedError -> {
                        Timber.e("Unable to sync all files: ${error.error}")
                        Result.failure()
                    }
                }
            } else {
                Result.success()
            }
        }
    }
}
