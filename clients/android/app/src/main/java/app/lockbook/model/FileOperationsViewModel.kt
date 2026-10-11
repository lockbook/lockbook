package app.lockbook.model

import android.content.ContentResolver
import android.net.Uri
import android.provider.DocumentsContract
import android.text.format.DateUtils
import android.webkit.MimeTypeMap
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.lockbook.File.FileType
import net.lockbook.Lb
import net.lockbook.LbError
import java.io.File
import java.io.IOException
import java.util.Locale

internal data class ExportResult(
    val saved: Int,
    val total: Int,
    val succeeded: Boolean,
)

internal sealed interface ExportState {
    data object Idle : ExportState

    data object Preparing : ExportState

    data class ChooseDestination(
        val selectedIds: List<String>,
        val singleDocument: net.lockbook.File?,
    ) : ExportState

    data object AwaitingDestination : ExportState

    data object Copying : ExportState

    data class Complete(
        val result: ExportResult,
    ) : ExportState

    data class Failed(
        val error: LbError? = null,
    ) : ExportState

    data object NoDocuments : ExportState
}

class FileOperationsViewModel(
    private val savedStateHandle: SavedStateHandle,
) : ViewModel() {
    private val mutableExport =
        MutableStateFlow<ExportState>(
            when {
                pendingExportIds.isNotEmpty() -> ExportState.AwaitingDestination
                savedStateHandle.get<Boolean>(EXPORT_IN_PROGRESS_KEY) == true -> ExportState.Failed()
                else -> ExportState.Idle
            },
        )
    internal val exporting = mutableExport.asStateFlow()

    private var pendingExportIds: List<String>
        get() = savedStateHandle.get<ArrayList<String>>(PENDING_EXPORT_IDS_KEY).orEmpty()
        set(ids) {
            savedStateHandle[PENDING_EXPORT_IDS_KEY] = ArrayList(ids)
        }
    private var pendingSingleDocument: Boolean
        get() = savedStateHandle[SINGLE_DOCUMENT_KEY] ?: false
        set(value) {
            savedStateHandle[SINGLE_DOCUMENT_KEY] = value
        }

    fun requestExport(selectedFiles: List<net.lockbook.File>) {
        if (mutableExport.value != ExportState.Idle) return
        val exportable = selectedFiles.filter { it.type != FileType.Link }
        if (exportable.isEmpty()) {
            mutableExport.value = ExportState.NoDocuments
            return
        }
        val singleDocument = exportable.singleOrNull()?.takeIf { it.type == FileType.Document }
        mutableExport.value = ExportState.ChooseDestination(exportable.map { it.id }, singleDocument)
    }

    internal fun takeExportPickerRequest(): ExportState.ChooseDestination? {
        val state = mutableExport.value as? ExportState.ChooseDestination ?: return null
        pendingExportIds = state.selectedIds
        pendingSingleDocument = state.singleDocument != null
        mutableExport.value = ExportState.AwaitingDestination
        return state
    }

    internal fun finishExport() {
        pendingExportIds = emptyList()
        pendingSingleDocument = false
        savedStateHandle[EXPORT_IN_PROGRESS_KEY] = false
        mutableExport.value = ExportState.Idle
    }

    internal fun exportPickerFailed() {
        pendingExportIds = emptyList()
        pendingSingleDocument = false
        mutableExport.value = ExportState.Failed()
    }

    private fun collectDocuments(
        selectedFiles: List<net.lockbook.File>,
        documents: MutableList<net.lockbook.File>,
    ) {
        selectedFiles.forEach { file ->
            when (file.type) {
                FileType.Document -> documents.add(file)
                FileType.Folder -> collectDocuments(Lb.getChildren(file.id).toList(), documents)
                FileType.Link -> Unit
            }
        }
    }

    private fun clearShareStorage(shareFolder: File) {
        val now = System.currentTimeMillis()
        shareFolder.listFiles()?.forEach { file ->
            val timestamp = file.name.toLongOrNull() ?: return@forEach
            if (now - timestamp > DateUtils.HOUR_IN_MILLIS) file.deleteRecursively()
        }
    }

    fun export(
        destination: Uri,
        resolver: ContentResolver,
        appDataDir: File,
    ) {
        if (mutableExport.value != ExportState.AwaitingDestination) return
        val ids = pendingExportIds
        if (ids.isEmpty()) return
        val singleDocument = pendingSingleDocument
        pendingExportIds = emptyList()
        pendingSingleDocument = false
        savedStateHandle[EXPORT_IN_PROGRESS_KEY] = true
        mutableExport.value = ExportState.Preparing
        viewModelScope.launch {
            try {
                val files = withContext(Dispatchers.IO) { prepareFiles(ids, appDataDir) }
                if (files.isEmpty()) {
                    if (singleDocument) withContext(Dispatchers.IO) { deleteCreatedDocument(resolver, destination) }
                    mutableExport.value = ExportState.NoDocuments
                    return@launch
                }
                mutableExport.value = ExportState.Copying
                mutableExport.value =
                    ExportState.Complete(
                        withContext(Dispatchers.IO) { copyFiles(files, destination, resolver, singleDocument) },
                    )
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: LbError) {
                if (singleDocument) withContext(Dispatchers.IO) { deleteCreatedDocument(resolver, destination) }
                mutableExport.value = ExportState.Failed(error)
            } catch (_: Exception) {
                if (singleDocument) withContext(Dispatchers.IO) { deleteCreatedDocument(resolver, destination) }
                mutableExport.value = ExportState.Failed()
            }
        }
    }

    private fun prepareFiles(ids: List<String>, appDataDir: File): List<File> {
        val shareRoot = File(appDataDir, "share")
        clearShareStorage(shareRoot)
        val documents = mutableListOf<net.lockbook.File>()
        collectDocuments(ids.map(Lb::getFileById), documents)
        val shareFolder = File(shareRoot, System.currentTimeMillis().toString()).apply { mkdirs() }
        return documents.distinctBy { it.id }.map { file ->
            val itemFolder = File(shareFolder, file.id).apply { mkdir() }
            Lb.exportFile(file.id, itemFolder.absolutePath, false)
            File(itemFolder, file.name).absoluteFile
        }
    }

    private fun copyFiles(
        files: List<File>,
        destination: Uri,
        resolver: ContentResolver,
        singleDocument: Boolean,
    ): ExportResult {
        var saved = 0
        try {
            if (singleDocument) {
                writeFile(resolver, files.single(), destination)
                saved = 1
            } else {
                val parent =
                    DocumentsContract.buildDocumentUriUsingTree(
                        destination,
                        DocumentsContract.getTreeDocumentId(destination),
                    )
                for (source in files) {
                    val created =
                        DocumentsContract.createDocument(resolver, parent, exportMimeType(source), source.name)
                            ?: throw IOException("Could not create ${source.name}")
                    try {
                        writeFile(resolver, source, created)
                    } catch (error: Exception) {
                        runCatching { DocumentsContract.deleteDocument(resolver, created) }
                        throw error
                    }
                    saved++
                }
            }
            return ExportResult(saved, files.size, succeeded = true)
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (_: Exception) {
            if (singleDocument) deleteCreatedDocument(resolver, destination)
            return ExportResult(saved, files.size, succeeded = false)
        }
    }

    private fun deleteCreatedDocument(resolver: ContentResolver, destination: Uri) {
        runCatching { DocumentsContract.deleteDocument(resolver, destination) }
    }

    private fun writeFile(
        resolver: ContentResolver,
        source: File,
        destination: Uri,
    ) {
        source.inputStream().use { input ->
            val output =
                resolver.openOutputStream(destination, "wt") ?: throw IOException("Could not open export destination")
            output.use(input::copyTo)
        }
    }

    companion object {
        private const val PENDING_EXPORT_IDS_KEY = "pending_export_ids"
        private const val SINGLE_DOCUMENT_KEY = "export_single_document"
        private const val EXPORT_IN_PROGRESS_KEY = "export_in_progress"
    }
}

internal fun exportMimeType(file: File): String =
    exportMimeType(file.name)

internal fun exportMimeType(fileName: String): String =
    MimeTypeMap.getSingleton().getMimeTypeFromExtension(fileName.substringAfterLast('.', "").lowercase(Locale.ROOT))
        ?: "application/octet-stream"
