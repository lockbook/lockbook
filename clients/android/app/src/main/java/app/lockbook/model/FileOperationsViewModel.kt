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

internal sealed interface OpenLinkState {
    data object Idle : OpenLinkState

    data object Running : OpenLinkState

    data class Complete(
        val result: OpenLinkResult,
    ) : OpenLinkState
}

internal data class OpenLinkResult(
    val fileId: String,
    val fileFound: Boolean = false,
    val error: LbError? = null,
)

internal data class ExportResult(
    val saved: Int,
    val total: Int,
    val succeeded: Boolean,
)

internal sealed interface ExportState {
    data object Idle : ExportState

    data object Preparing : ExportState

    data class ChooseDestination(
        val files: List<File>,
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
    private val mutableOpening = MutableStateFlow<OpenLinkState>(OpenLinkState.Idle)
    internal val opening = mutableOpening.asStateFlow()
    private val mutableExport =
        MutableStateFlow<ExportState>(
            when {
                pendingExportFiles.isNotEmpty() -> ExportState.AwaitingDestination
                savedStateHandle.get<Boolean>(EXPORT_IN_PROGRESS_KEY) == true -> ExportState.Failed()
                else -> ExportState.Idle
            },
        )
    internal val exporting = mutableExport.asStateFlow()

    private var pendingExportFiles: List<File>
        get() = savedStateHandle.get<ArrayList<String>>(PENDING_EXPORT_FILES_KEY)?.map(::File).orEmpty()
        set(files) {
            savedStateHandle[PENDING_EXPORT_FILES_KEY] = ArrayList(files.map { it.absolutePath })
        }

    fun open(fileId: String) {
        if (mutableOpening.value != OpenLinkState.Idle) return
        mutableOpening.value = OpenLinkState.Running
        viewModelScope.launch {
            mutableOpening.value =
                OpenLinkState.Complete(
                    withContext(Dispatchers.IO) {
                        resolveOpenLink(
                            fileId,
                            sync = { Lb.sync() },
                            fileExists = { Lb.getFileById(it) != null },
                        )
                    },
                )
        }
    }

    internal fun acknowledgeOpening() {
        if (mutableOpening.value is OpenLinkState.Complete) mutableOpening.value = OpenLinkState.Idle
    }

    fun prepareExport(
        selectedFiles: List<net.lockbook.File>,
        appDataDir: File,
    ) {
        if (mutableExport.value != ExportState.Idle) return
        savedStateHandle[EXPORT_IN_PROGRESS_KEY] = true
        mutableExport.value = ExportState.Preparing
        viewModelScope.launch {
            try {
                val exported =
                    withContext(Dispatchers.IO) {
                        val shareRoot = File(appDataDir, "share")
                        clearShareStorage(shareRoot)
                        val documents = mutableListOf<net.lockbook.File>()
                        collectDocuments(selectedFiles, documents)
                        val shareFolder = File(shareRoot, System.currentTimeMillis().toString()).apply { mkdirs() }
                        documents.distinctBy { it.id }.map { file ->
                            val itemFolder = File(shareFolder, file.id).apply { mkdir() }
                            Lb.exportFile(file.id, itemFolder.absolutePath, false)
                            File(itemFolder, file.name).absoluteFile
                        }
                    }
                mutableExport.value = if (exported.isEmpty()) ExportState.NoDocuments else ExportState.ChooseDestination(exported)
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: LbError) {
                mutableExport.value = ExportState.Failed(error)
            } catch (_: Exception) {
                mutableExport.value = ExportState.Failed()
            }
        }
    }

    internal fun takeExportPickerRequest(): List<File>? {
        val state = mutableExport.value as? ExportState.ChooseDestination ?: return null
        pendingExportFiles = state.files
        mutableExport.value = ExportState.AwaitingDestination
        return state.files
    }

    internal fun finishExport() {
        pendingExportFiles = emptyList()
        savedStateHandle[EXPORT_IN_PROGRESS_KEY] = false
        mutableExport.value = ExportState.Idle
    }

    internal fun exportPickerFailed() {
        pendingExportFiles = emptyList()
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
    ) {
        if (mutableExport.value != ExportState.AwaitingDestination) return
        val files = pendingExportFiles
        if (files.isEmpty()) return
        pendingExportFiles = emptyList()
        mutableExport.value = ExportState.Copying
        viewModelScope.launch {
            mutableExport.value =
                ExportState.Complete(
                    withContext(Dispatchers.IO) {
                        var saved = 0
                        try {
                            if (files.size == 1) {
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
                            ExportResult(saved, files.size, succeeded = true)
                        } catch (cancelled: CancellationException) {
                            throw cancelled
                        } catch (_: Exception) {
                            ExportResult(saved, files.size, succeeded = false)
                        }
                    },
                )
        }
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
        private const val PENDING_EXPORT_FILES_KEY = "pending_export_files"
        private const val EXPORT_IN_PROGRESS_KEY = "export_in_progress"
    }
}

internal fun exportMimeType(file: File): String =
    MimeTypeMap.getSingleton().getMimeTypeFromExtension(file.extension.lowercase(Locale.ROOT)) ?: "application/octet-stream"

internal fun resolveOpenLink(
    fileId: String,
    sync: () -> Unit,
    fileExists: (String) -> Boolean,
): OpenLinkResult {
    val syncError =
        try {
            sync()
            null
        } catch (error: LbError) {
            error
        }
    val found =
        try {
            fileExists(fileId)
        } catch (_: LbError) {
            false
        }
    return OpenLinkResult(fileId, fileFound = found, error = syncError)
}
