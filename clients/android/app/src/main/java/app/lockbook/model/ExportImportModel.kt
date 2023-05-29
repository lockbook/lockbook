package app.lockbook.model

import android.text.format.DateUtils
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.Result
import java.io.File
import kotlin.collections.ArrayList

class ExportImportModel(
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

    fun exportDocuments(selectedFiles: List<app.lockbook.util.File>, appDataDir: File): Result<Unit, CoreError<out UiCoreError>> {
        val cacheDir = getMainShareFolder(appDataDir)

        isLoadingOverlayVisible = true
        _updateMainScreenUI.postValue(UpdateMainScreenUI.ShowHideProgressOverlay(isLoadingOverlayVisible))

        clearShareStorage(cacheDir)

        val documents = mutableListOf<app.lockbook.util.File>()
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

            if (file.name.endsWith(".draw")) {
                val image = File(
                    shareItemFolder,
                    file.name.removeSuffix(".draw") + ".${IMAGE_EXPORT_TYPE.name.lowercase()}"
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
                when (val exportFileResult = CoreModel.exportFile(file.id, shareItemFolder.absolutePath, false)) {
                    is Ok -> filesToShare.add(
                        File(
                            shareItemFolder,
                            file.name
                        ).absoluteFile
                    )
                    is Err -> {
                        isLoadingOverlayVisible = false
                        _updateMainScreenUI.postValue(UpdateMainScreenUI.ShowHideProgressOverlay(isLoadingOverlayVisible))
                        return exportFileResult
                    }
                }
            }
        }

        _updateMainScreenUI.postValue(UpdateMainScreenUI.ShareDocuments(filesToShare))
        return Ok(Unit)
    }

    private fun retrieveSelectedDocuments(
        selectedFiles: List<app.lockbook.util.File>,
        documents: MutableList<app.lockbook.util.File>
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
