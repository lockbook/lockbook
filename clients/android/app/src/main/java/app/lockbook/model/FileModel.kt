package app.lockbook.model

import android.content.Context
import androidx.preference.PreferenceManager
import app.lockbook.App.Companion.config
import app.lockbook.R
import app.lockbook.util.*
import com.github.michaelbull.result.*

enum class SortStyle {
    AToZ,
    ZToA,
    LastChanged,
    FirstChanged,
    FileType
}

class FileModel(var parent: ClientFileMetadata, var children: List<ClientFileMetadata>, val fileDir: MutableList<ClientFileMetadata>, private var sortStyle: SortStyle) {
    companion object {
        fun createAtRoot(context: Context): Result<FileModel, LbError> {
            val pref = PreferenceManager.getDefaultSharedPreferences(context)
            val res = context.resources

            val sortStyle = when (
                pref.getString(
                    getString(res, R.string.sort_files_key),
                    getString(res, R.string.sort_files_a_z_value)
                )
            ) {
                getString(res, R.string.sort_files_a_z_value) -> SortStyle.AToZ
                getString(res, R.string.sort_files_z_a_value) -> SortStyle.ZToA
                getString(res, R.string.sort_files_first_changed_value) -> SortStyle.FirstChanged
                getString(res, R.string.sort_files_last_changed_value) -> SortStyle.LastChanged
                getString(res, R.string.sort_files_type_value) -> SortStyle.FileType
                else -> return Err(LbError.basicError(context.resources))
            }

            return when (val getRootResult = CoreModel.getRoot(config)) {
                is Ok -> {
                    val root = getRootResult.value
                    when (val getChildrenResult = CoreModel.getChildren(config, root.id)) {
                        is Ok -> {
                            val fileModel = FileModel(root, getChildrenResult.value, mutableListOf(root), sortStyle)
                            fileModel.sortChildren()

                            Ok(fileModel)
                        }
                        is Err -> Err(getChildrenResult.error.toLbError(res))
                    }
                }
                is Err -> Err(getRootResult.error.toLbError(res))
            }
        }

        fun deleteFiles(ids: List<String>): Result<Unit, CoreError> {
            for (id in ids) {
                val deleteFileResult = CoreModel.deleteFile(config, id)
                if (deleteFileResult is Err) {
                    return deleteFileResult
                }
            }

            return Ok(Unit)
        }
    }

    fun refreshChildrenAtPastParent(position: Int): Result<Unit, GetChildrenError> {
        val firstChildPosition = position + 1
        for (index in firstChildPosition until fileDir.size) {
            fileDir.removeAt(firstChildPosition)
        }

        parent = fileDir.last()
        return refreshChildren()
    }

    fun isAtRoot(): Boolean = parent.id == parent.parent

    fun setSortStyle(newSortStyle: SortStyle) {
        sortStyle = newSortStyle
        sortChildren()
    }

    private fun sortChildren() {
        children = when (sortStyle) {
            SortStyle.AToZ -> sortFilesAlpha(children)
            SortStyle.ZToA -> sortFilesAlpha(children).reversed()
            SortStyle.LastChanged -> sortFilesChanged(children)
            SortStyle.FirstChanged -> sortFilesChanged(children).reversed()
            SortStyle.FileType -> sortFilesType(children)
        }
    }

    private fun refreshChildrenAtNewParent(newParent: ClientFileMetadata): Result<Unit, GetChildrenError> {
        val oldParent = parent
        parent = newParent

        val refreshChildrenResult = refreshChildren()
        if (refreshChildrenResult is Err) {
            parent = oldParent
        }

        return refreshChildrenResult
    }

    fun refreshChildren(): Result<Unit, GetChildrenError> {
        return CoreModel.getChildren(config, parent.id).map { newChildren ->
            children = newChildren.filter { fileMetadata -> fileMetadata.id != fileMetadata.parent }
            sortChildren()
        }
    }

    fun intoChild(newParent: ClientFileMetadata): Result<Unit, GetChildrenError> {
        return refreshChildrenAtNewParent(newParent).map {
            fileDir.add(newParent)
        }
    }

    fun intoParent(): Result<Unit, CoreError> {
        return CoreModel.getFileById(config, parent.parent).andThen { newParent ->
            refreshChildrenAtNewParent(newParent).map {
                if (fileDir.size != 1) {
                    fileDir.removeLastOrNull()
                }
            }
        }
    }

    private fun sortFilesAlpha(files: List<ClientFileMetadata>): List<ClientFileMetadata> =
        files.sortedBy { fileMetadata ->
            fileMetadata.name
        }

    private fun sortFilesChanged(files: List<ClientFileMetadata>): List<ClientFileMetadata> = files.sortedBy { fileMetadata ->
        fileMetadata.metadataVersion
    }

    private fun sortFilesType(files: List<ClientFileMetadata>): List<ClientFileMetadata> {
        val tempFolders = files.filter { fileMetadata ->
            fileMetadata.fileType.name == FileType.Folder.name
        }
        val tempDocuments = files.filter { fileMetadata ->
            fileMetadata.fileType.name == FileType.Document.name
        }

        return tempFolders.union(
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
    }
}
