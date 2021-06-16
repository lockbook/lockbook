package app.lockbook.model

import android.text.format.DateUtils
import app.lockbook.App
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import timber.log.Timber
import java.io.File
import java.util.*
import kotlin.collections.ArrayList

class ShareModel(
    private val config: Config,
    private val _shareDocument: SingleMutableLiveData<ArrayList<File>>,
    private val _showHideProgressOverlay: SingleMutableLiveData<Boolean>,
    private val _errorHasOccurred: SingleMutableLiveData<String>,
    private val _unexpectedErrorHasOccurred: SingleMutableLiveData<String>
) {
    var isLoadingOverlayVisible = false

    companion object {
        fun getMainShareFolder(): File = File(App.instance.applicationContext.cacheDir, "share/")
        fun createRandomShareFolderInstance(): File = File(getMainShareFolder(), System.currentTimeMillis().toString())

        fun clearShareStorage() {
            val shareFolder = getMainShareFolder()
            val timeNow = System.currentTimeMillis()

            shareFolder.listFiles { file ->
                val timeThen = file.name.toLongOrNull() ?: return@listFiles false

                if ((timeNow - timeThen) > DateUtils.HOUR_IN_MILLIS) {
                    file.deleteRecursively()
                }

                true
            }
        }
    }

    fun shareDocuments(selectedFiles: List<FileMetadata>) {
        isLoadingOverlayVisible = true
        _showHideProgressOverlay.postValue(isLoadingOverlayVisible)

        clearShareStorage()

        val documents = mutableListOf<FileMetadata>()
        retrieveSelectedDocuments(selectedFiles, documents)

        val filesToShare = ArrayList<File>()
        val shareFolder = createRandomShareFolderInstance()
        shareFolder.mkdirs()

        for (file in documents) {
            val shareItemFolder = File(
                shareFolder,
                file.id
            ).absoluteFile

            shareItemFolder.mkdir()

            if (file.name.endsWith(".draw")) {
                val image = File(
                    shareItemFolder,
                    file.name.removeSuffix(".draw") + ".${IMAGE_EXPORT_TYPE.name.lowercase()}"
                ).absoluteFile

                when (
                    val exportDrawingToDiskResult =
                        CoreModel.exportDrawingToDisk(config, file.id, SupportedImageFormats.Jpeg, image.absolutePath)
                ) {
                    is Ok -> filesToShare.add(image)
                    is Err -> {
                        isLoadingOverlayVisible = false
                        _showHideProgressOverlay.postValue(isLoadingOverlayVisible)

                        return when (val error = exportDrawingToDiskResult.error) {
                            ExportDrawingToDiskError.FileDoesNotExist -> _errorHasOccurred.postValue("Error! File does not exist!")
                            ExportDrawingToDiskError.FolderTreatedAsDrawing -> _errorHasOccurred.postValue("Error! Folder treated as document!")
                            ExportDrawingToDiskError.InvalidDrawing -> _errorHasOccurred.postValue("Error! Invalid drawing!")
                            ExportDrawingToDiskError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                            ExportDrawingToDiskError.BadPath -> _errorHasOccurred.postValue("Error! Bad path used!")
                            ExportDrawingToDiskError.FileAlreadyExistsInDisk -> _errorHasOccurred.postValue("Error! File already exists in path!")
                            is ExportDrawingToDiskError.Unexpected -> {
                                Timber.e(error.error)
                                _unexpectedErrorHasOccurred.postValue(error.error)
                            }
                        }.exhaustive
                    }
                }
            } else {
                val doc = File(
                    shareItemFolder,
                    file.name
                ).absoluteFile

                when (val saveDocumentToDiskResult = CoreModel.saveDocumentToDisk(config, file.id, doc.absolutePath)) {
                    is Ok -> filesToShare.add(doc)
                    is Err -> {
                        isLoadingOverlayVisible = false
                        _showHideProgressOverlay.postValue(isLoadingOverlayVisible)

                        return when (val error = saveDocumentToDiskResult.error) {
                            SaveDocumentToDiskError.TreatedFolderAsDocument -> _errorHasOccurred.postValue(
                                "Error! Folder treated as document!"
                            )
                            SaveDocumentToDiskError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                            SaveDocumentToDiskError.FileDoesNotExist -> _errorHasOccurred.postValue("Error! File does not exist!")
                            SaveDocumentToDiskError.BadPath -> _errorHasOccurred.postValue("Error! Bad path used!")
                            SaveDocumentToDiskError.FileAlreadyExistsInDisk -> _errorHasOccurred.postValue("Error! File already exists in path!")
                            is SaveDocumentToDiskError.Unexpected -> {
                                Timber.e("Unable to get content of file: ${error.error}")
                                _unexpectedErrorHasOccurred.postValue(
                                    error.error
                                )
                            }
                        }.exhaustive
                    }
                }
            }
        }

        _shareDocument.postValue(filesToShare)
    }

    private fun retrieveSelectedDocuments(
        selectedFiles: List<FileMetadata>,
        documents: MutableList<FileMetadata>
    ): Boolean {
        selectedFiles.forEach { file ->
            when (file.fileType) {
                FileType.Document -> documents.add(file)
                FileType.Folder ->
                    when (
                        val getChildrenResult =
                            CoreModel.getChildren(config, file.id)
                    ) {
                        is Ok -> if (!retrieveSelectedDocuments(getChildrenResult.value, documents)) {
                            return false
                        }
                        is Err -> when (val error = getChildrenResult.error) {
                            is GetChildrenError.Unexpected -> {
                                Timber.e("Unable to get siblings of the parent: ${error.error}")
                                _unexpectedErrorHasOccurred.postValue(error.error)
                            }
                        }.exhaustive
                    }
            }
        }

        return true
    }
}
