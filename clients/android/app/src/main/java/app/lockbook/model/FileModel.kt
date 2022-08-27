package app.lockbook.model

import android.content.Context
import app.lockbook.util.*
import com.github.michaelbull.result.*

class FileModel(
    var parent: File,
    var files: List<File>,
    var children: List<File>,
    var recentFiles: List<File>,
    val fileDir: MutableList<File>,
) {

    companion object {
        // Returns Ok(null) if there is no root
        fun createAtRoot(context: Context): Result<FileModel?, LbError> {
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
                                listOf(),
                                recentFiles,
                                mutableListOf(root),
                            )
                            fileModel.refreshChildren()

                            Ok(fileModel)
                        }
                        is Err -> Err(listMetadatasResult.error.toLbError(context.resources))
                    }
                }
                is Err -> {
                    if ((getRootResult.error as? CoreError.UiError)?.content == GetRootError.NoRoot) {
                        Ok(null)
                    } else {
                        Err(getRootResult.error.toLbError(context.resources))
                    }
                }
            }
        }

        private fun getTenMostRecentFiles(files: List<File>): List<File> {
            val intermediateRecentFiles =
                files.asSequence().filter { it.fileType == FileType.Document }
                    .sortedBy { it.lastModified }.toList()

            val recentFiles = try {
                intermediateRecentFiles.takeLast(10)
            } catch (e: Exception) {
                intermediateRecentFiles
            }

            return recentFiles.reversed()
        }

        fun sortFiles(files: List<File>): List<File> {
            val folders = files.filter { fileMetadata ->
                fileMetadata.fileType == FileType.Folder
            }

            val documents = files.filter { fileMetadata ->
                fileMetadata.fileType == FileType.Document
            }

            return folders.sortedBy { it.name } + documents.sortedBy { it.name }
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

    fun refreshFiles(): Result<Unit, CoreError<Empty>> {
        return CoreModel.listMetadatas().map { files ->
            this.files = files
            recentFiles = getTenMostRecentFiles(files)
            refreshChildren()
        }
    }

    fun intoFile(newParent: File) {
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

    private fun sortChildren() {
        children = sortFiles(children)
    }
}
