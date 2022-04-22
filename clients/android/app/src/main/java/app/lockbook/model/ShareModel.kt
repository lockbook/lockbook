package app.lockbook.model

import android.text.format.DateUtils
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.Result
import java.io.File
import java.util.*
import kotlin.collections.ArrayList

class ShareModel(
    private val _updateMainScreenUI: SingleMutableLiveData<UpdateMainScreenUI>
) {
    var isLoadingOverlayVisible = false

    companion object {
        private fun getMainShareFolder(cacheDir: File): File = File(cacheDir, "share/")
        fun createRandomShareFolderInstance(cacheDir: File): File = File(getMainShareFolder(cacheDir), System.currentTimeMillis().toString())

        fun clearShareStorage(cacheDir: File) {
            val shareFolder = getMainShareFolder(cacheDir)
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

    fun shareDocuments(selectedFiles: List<DecryptedFileMetadata>, appDataDir: File): Result<Unit, CoreError<out UiCoreError>> {
        val cacheDir = getMainShareFolder(appDataDir)

        isLoadingOverlayVisible = true
        _updateMainScreenUI.postValue(UpdateMainScreenUI.ShowHideProgressOverlay(isLoadingOverlayVisible))

        clearShareStorage(cacheDir)

        val documents = mutableListOf<DecryptedFileMetadata>()
        val selectedDocumentsResult = retrieveSelectedDocuments(selectedFiles, documents)
        if (selectedDocumentsResult is Err) {
            return selectedDocumentsResult
        }

        val filesToShare = ArrayList<File>()
        val shareFolder = createRandomShareFolderInstance(cacheDir)
        shareFolder.mkdirs()

        for (file in documents) {
            val shareItemFolder = File(
                shareFolder,
                file.id
            ).absoluteFile

            shareItemFolder.mkdir()

            if (file.decryptedName.endsWith(".draw")) {
                val image = File(
                    shareItemFolder,
                    file.decryptedName.removeSuffix(".draw") + ".${IMAGE_EXPORT_TYPE.name.lowercase()}"
                ).absoluteFile

                when (
                    val exportDrawingToDiskResult =
                        CoreModel.exportDrawingToDisk(file.id, SupportedImageFormats.Jpeg, image.absolutePath)
                ) {
                    is Ok -> filesToShare.add(image)
                    is Err -> {
                        isLoadingOverlayVisible = false
                        _updateMainScreenUI.postValue(UpdateMainScreenUI.ShowHideProgressOverlay(isLoadingOverlayVisible))
                        return exportDrawingToDiskResult
                    }
                }
            } else {
                val doc = File(
                    shareItemFolder,
                    file.decryptedName
                ).absoluteFile

                when (val saveDocumentToDiskResult = CoreModel.saveDocumentToDisk(file.id, doc.absolutePath)) {
                    is Ok -> filesToShare.add(doc)
                    is Err -> {
                        isLoadingOverlayVisible = false
                        _updateMainScreenUI.postValue(UpdateMainScreenUI.ShowHideProgressOverlay(isLoadingOverlayVisible))
                        return saveDocumentToDiskResult
                    }
                }
            }
        }

        _updateMainScreenUI.postValue(UpdateMainScreenUI.ShareDocuments(filesToShare))
        return Ok(Unit)
    }

    private fun retrieveSelectedDocuments(
        selectedFiles: List<DecryptedFileMetadata>,
        documents: MutableList<DecryptedFileMetadata>
    ): Result<Unit, CoreError<out UiCoreError>> {
        for (file in selectedFiles) {
            when (file.fileType) {
                FileType.Document -> {
                    documents.add(file)
                }
                FileType.Folder ->
                    when (
                        val getChildrenResult =
                            CoreModel.getChildren(file.id)
                    ) {
                        is Ok -> {
                            val retrieveDocumentsResult = retrieveSelectedDocuments(getChildrenResult.value, documents)
                            if (retrieveDocumentsResult is Err) {
                                return retrieveDocumentsResult
                            }
                        }
                        is Err -> return getChildrenResult
                    }
            }
        }

        return Ok(Unit)
    }
}
