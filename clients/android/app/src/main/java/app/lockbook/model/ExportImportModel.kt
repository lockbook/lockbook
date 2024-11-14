package app.lockbook.model

import android.text.format.DateUtils
import app.lockbook.util.*
import net.lockbook.File.FileType
import net.lockbook.Lb
import net.lockbook.LbError
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

    fun exportDocuments(selectedFiles: List<net.lockbook.File>, appDataDir: File) {
        val cacheDir = getMainShareFolder(appDataDir)

        isLoadingOverlayVisible = true
        _updateMainScreenUI.postValue(UpdateMainScreenUI.ShowHideProgressOverlay(isLoadingOverlayVisible))

        clearShareStorage(cacheDir)

        val documents = mutableListOf<net.lockbook.File>()
        retrieveSelectedDocuments(selectedFiles, documents)

        val filesToShare = ArrayList<File>()
        val shareFolder = createRandomShareFolderInstance(cacheDir)
        shareFolder.mkdirs()

        for (file in documents) {
            val shareItemFolder = File(
                shareFolder,
                file.id
            ).absoluteFile

            shareItemFolder.mkdir()

            try {
                Lb.exportFile(file.id, shareItemFolder.absolutePath, false)

                filesToShare.add(
                    File(
                        shareItemFolder,
                        file.name
                    ).absoluteFile
                )
            } catch (err: LbError) {
                isLoadingOverlayVisible = false
                _updateMainScreenUI.postValue(
                    UpdateMainScreenUI.ShowHideProgressOverlay(
                        isLoadingOverlayVisible
                    )
                )
                throw err
            }
        }

        _updateMainScreenUI.postValue(UpdateMainScreenUI.ShareDocuments(filesToShare))
    }

    private fun retrieveSelectedDocuments(
        selectedFiles: List<net.lockbook.File>,
        documents: MutableList<net.lockbook.File>
    ) {
        for (file in selectedFiles) {
            when (file.type) {
                FileType.Document -> {
                    documents.add(file)
                }
                FileType.Folder -> {
                    val children = Lb.getChildren(file.id)
                    retrieveSelectedDocuments(children.toList(), documents)
                }
                FileType.Link -> {} // won't happen
            }
        }
    }
}
