package app.lockbook.model

import androidx.lifecycle.MutableLiveData
import app.lockbook.App
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import timber.log.Timber
import java.io.File

class ShareModel(
    private val config: Config,
    private val _shareDocument: SingleMutableLiveData<ArrayList<File>>,
    private val _showHideProgressOverlay: MutableLiveData<Boolean>,
    private val _errorHasOccurred: SingleMutableLiveData<String>,
    private val _unexpectedErrorHasOccurred: SingleMutableLiveData<String>
) {
    fun shareDocument(selectedFiles: List<FileMetadata>) {
        _showHideProgressOverlay.postValue(true)

        val documents = mutableListOf<FileMetadata>()
        getFilesToShare(selectedFiles, documents)

        val files = ArrayList<File>()
        val imagesPath = File(App.instance.applicationContext.cacheDir, "images/")
        imagesPath.mkdirs()

        val docsPath = File(App.instance.applicationContext.cacheDir, "docs/")
        docsPath.mkdirs()

        for (file in documents) {
            if (file.name.endsWith(".draw")) {
                when (
                    val exportDrawingResult =
                        CoreModel.exportDrawing(config, file.id, SupportedImageFormats.Jpeg)
                ) {
                    is Ok -> {
                        val image = File(
                            imagesPath,
                            file.name.replace(".draw", ".${IMAGE_EXPORT_TYPE.name.lowercase()}")
                        )
                        image.createNewFile()
                        image.writeBytes(exportDrawingResult.value.toByteArray())
                        files.add(image)
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
                        val doc = File(docsPath, file.name)
                        doc.createNewFile()
                        doc.writeText(readDocumentResult.value)
                        files.add(doc)
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

        _shareDocument.postValue(files)
    }

    private fun getFilesToShare(
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
                        is Ok -> if (!getFilesToShare(getChildrenResult.value, documents)) {
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
