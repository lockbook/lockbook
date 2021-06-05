package app.lockbook.model

import app.lockbook.App
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import timber.log.Timber
import java.io.File

class ShareModel(
    private val config: Config,
    private val _shareDocument: SingleMutableLiveData<ArrayList<File>>,
    private val _showHideProgressOverlay: SingleMutableLiveData<Boolean>,
    private val _errorHasOccurred: SingleMutableLiveData<String>,
    private val _unexpectedErrorHasOccurred: SingleMutableLiveData<String>
) {
    var isLoadingOverlayVisible = false

    private fun getShareStorageFolders(): List<File> = listOf(
        File(App.instance.applicationContext.cacheDir, "images/"),
        File(App.instance.applicationContext.cacheDir, "docs/")
    )

    fun clearStorage() {
        val (imagesFolder, docsFolder) = getShareStorageFolders()

        imagesFolder.deleteRecursively()
        docsFolder.deleteRecursively()
    }

    fun shareDocument(selectedFiles: List<FileMetadata>) {
        isLoadingOverlayVisible = true
        _showHideProgressOverlay.postValue(isLoadingOverlayVisible)

        val documents = mutableListOf<FileMetadata>()
        retreiveSelectedDocuments(selectedFiles, documents)

        val filesToShare = ArrayList<File>()
        val (imagesFolder, docsFolder) = getShareStorageFolders()
        imagesFolder.mkdirs()
        docsFolder.mkdirs()

        for (file in documents) {
            if (file.name.endsWith(".draw")) {
                when (
                    val exportDrawingResult =
                        CoreModel.exportDrawing(config, file.id, SupportedImageFormats.Jpeg)
                ) {
                    is Ok -> {
                        val image = File(
                            imagesFolder,
                            file.name.replace(".draw", ".${IMAGE_EXPORT_TYPE.name.lowercase()}")
                        )
                        image.createNewFile()
                        image.writeBytes(exportDrawingResult.value.toByteArray())
                        filesToShare.add(image)
                    }
                    is Err -> return when (val error = exportDrawingResult.error) {
                        ExportDrawingError.FileDoesNotExist -> _errorHasOccurred.postValue("Error! File does not exist!")
                        ExportDrawingError.FolderTreatedAsDrawing -> _errorHasOccurred.postValue("Error! Folder treated as document!")
                        ExportDrawingError.InvalidDrawing -> _errorHasOccurred.postValue("Error! Invalid drawing!")
                        ExportDrawingError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                        is ExportDrawingError.Unexpected -> {
                            Timber.e(error.error)
                            _unexpectedErrorHasOccurred.postValue(error.error)
                        }
                    }.exhaustive
                }
            } else {
                when (val readDocumentResult = CoreModel.readDocument(config, file.id)) {
                    is Ok -> {
                        val doc = File(docsFolder, file.name)
                        doc.createNewFile()
                        doc.writeText(readDocumentResult.value)
                        filesToShare.add(doc)
                    }
                    is Err -> return when (val error = readDocumentResult.error) {
                        is ReadDocumentError.TreatedFolderAsDocument -> _errorHasOccurred.postValue(
                            "Error! Folder treated as document!"
                        )
                        is ReadDocumentError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                        is ReadDocumentError.FileDoesNotExist -> _errorHasOccurred.postValue("Error! File does not exist!")
                        is ReadDocumentError.Unexpected -> {
                            Timber.e("Unable to get content of file: ${error.error}")
                            _unexpectedErrorHasOccurred.postValue(
                                error.error
                            )
                        }
                    }.exhaustive
                }
            }
        }

        _shareDocument.postValue(filesToShare)
    }

    private fun retreiveSelectedDocuments(
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
                        is Ok -> if (!retreiveSelectedDocuments(getChildrenResult.value, documents)) {
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
