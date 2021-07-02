package app.lockbook.model

import android.text.format.DateUtils
import app.lockbook.App
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import java.io.File
import java.util.*
import kotlin.collections.ArrayList

class ShareModel(
    private val config: Config,
    private val _shareDocument: SingleMutableLiveData<ArrayList<File>>,
    private val _showHideProgressOverlay: SingleMutableLiveData<Boolean>,
    private val _notifyError: SingleMutableLiveData<LbError>
) {
    var isLoadingOverlayVisible = false

    companion object {
        private fun getMainShareFolder(): File = File(App.instance.applicationContext.cacheDir, "share/")
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

    fun shareDocuments(selectedFiles: List<ClientFileMetadata>) {
        isLoadingOverlayVisible = true
        _showHideProgressOverlay.postValue(isLoadingOverlayVisible)

        clearShareStorage()

        val documents = mutableListOf<ClientFileMetadata>()
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
                        return _notifyError.postValue(exportDrawingToDiskResult.error.toLbError())
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
                        return _notifyError.postValue(saveDocumentToDiskResult.error.toLbError())
                    }
                }
            }
        }

        _shareDocument.postValue(filesToShare)
    }

    private fun retrieveSelectedDocuments(
        selectedFiles: List<ClientFileMetadata>,
        documents: MutableList<ClientFileMetadata>
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
                        is Err -> _notifyError.postValue(getChildrenResult.error.toLbError())
                    }
            }
        }

        return true
    }
}
