package app.lockbook.model

import android.content.Context
import androidx.preference.PreferenceManager
import app.lockbook.R
import app.lockbook.util.*
import com.github.michaelbull.result.*

enum class SortStyle {
    AToZ,
    ZToA,
    LastChanged,
    FirstChanged,
    FileType;

    fun toStringResource(): Int = when (this) {
        AToZ -> R.string.sort_files_a_z_value
        ZToA -> R.string.sort_files_z_a_value
        LastChanged -> R.string.sort_files_last_changed_value
        FirstChanged -> R.string.sort_files_first_changed_value
        FileType -> R.string.sort_files_type_value
    }
}

class FileModel(
    var parent: DecryptedFileMetadata,
    var files: List<DecryptedFileMetadata>,
    var children: List<DecryptedFileMetadata>,
    var recentFiles: List<DecryptedFileMetadata>,
    val fileDir: MutableList<DecryptedFileMetadata>,
    private var sortStyle: SortStyle
) {

    companion object {
        // Returns Ok(null) if there is no root
        fun createAtRoot(context: Context): Result<FileModel?, LbError> {
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

            return when (val getRootResult = CoreModel.getRoot()) {
                is Ok -> {
                    when (val listMetadatasResult = CoreModel.listMetadatas()) {
                        is Ok -> {
                            val root = getRootResult.value
                            val files = listMetadatasResult.value
                            val recentFiles = getTenMostRecentFiles(files)

                            val fileModel = FileModel(
                                root,
                                files,
                                files.filter { it.parent == root.id && it.id != it.parent },
                                recentFiles,
                                mutableListOf(root),
                                sortStyle
                            )
                            fileModel.sortChildren()

                            Ok(fileModel)
                        }
                        is Err -> Err(listMetadatasResult.error.toLbError(res))
                    }
                }
                is Err -> {
                    if ((getRootResult.error as? CoreError.UiError)?.content == GetRootError.NoRoot) {
                        Ok(null)
                    } else {
                        Err(getRootResult.error.toLbError(res))
                    }
                }
            }
        }

        private fun getTenMostRecentFiles(files: List<DecryptedFileMetadata>): List<DecryptedFileMetadata> {
            val intermediateRecentFiles =
                files.asSequence().filter { it.fileType == FileType.Document }
                    .sortedBy { it.contentVersion }.toList()

            val recentFiles = try {
                intermediateRecentFiles.takeLast(10)
            } catch (e: Exception) {
                intermediateRecentFiles
            }

            return recentFiles.reversed()
        }
    }

    fun refreshChildrenAtAncestor(position: Int) {
        val firstChildPosition = position + 1
        for (index in firstChildPosition until fileDir.size) {
            fileDir.removeAt(firstChildPosition)
        }

        parent = fileDir.last()
        refreshChildren()
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

    fun refreshFiles(): Result<Unit, CoreError<Empty>> {
        return CoreModel.listMetadatas().map { files ->
            this.files = files
            recentFiles = getTenMostRecentFiles(files)
            refreshChildren()
        }
    }

    fun intoFile(newParent: DecryptedFileMetadata) {
        parent = newParent
        refreshChildren()
        fileDir.add(newParent)
    }

    fun intoParent() {
        parent = files.filter { it.id == parent.parent }[0]
        refreshChildren()
        fileDir.removeLast()
    }

    fun refreshChildren() {
        children = files.filter { it.parent == parent.id && it.id != it.parent }
        sortChildren()
    }

    private fun sortFilesAlpha(files: List<DecryptedFileMetadata>): List<DecryptedFileMetadata> =
        files.sortedBy { fileMetadata ->
            fileMetadata.decryptedName
        }

    private fun sortFilesChanged(files: List<DecryptedFileMetadata>): List<DecryptedFileMetadata> =
        files.sortedBy { fileMetadata ->
            fileMetadata.metadataVersion
        }

    private fun sortFilesType(files: List<DecryptedFileMetadata>): List<DecryptedFileMetadata> {
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
                        Regex(".[^.]+\$").find(fileMetadata.decryptedName)?.value
                    },
                    { fileMetaData ->
                        fileMetaData.decryptedName
                    }
                )
            )
        ).toList()
    }
}
