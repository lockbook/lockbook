package app.lockbook.model

import android.content.Context
import app.lockbook.util.*
import com.github.michaelbull.result.*

class FileModel(
    val root: File,
    var parent: File,
    var idsAndFiles: Map<String, File>,
    var children: List<File>,
    var suggestedDocs: List<File>,
    val fileDir: MutableList<File>,
) {

    companion object {
        // Returns Ok(null) if there is no root
        fun createAtRoot(context: Context): Result<FileModel?, LbError> {
            return when (val getRootResult = CoreModel.getRoot()) {
                is Ok -> {
                    val root = getRootResult.value

                    val fileModel = FileModel(
                        root,
                        root,
                        emptyMap(),
                        listOf(),
                        listOf(),
                        mutableListOf(root),
                    )
                    fileModel.refreshFiles()

                    Ok(fileModel)
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

        private fun suggestedDocs(idsAndFiles: Map<String, File>): Result<List<File>, CoreError<Empty>> {
            return when (val suggestedDocsResult = CoreModel.suggestedDocs()) {
                is Ok -> {
                    Ok(
                        suggestedDocsResult.value.filter {
                            idsAndFiles.containsKey(it)
                        }.map {
                            idsAndFiles[it]!!
                        }
                    )
                }
                is Err -> {
                    Err(suggestedDocsResult.error)
                }
            }
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
            this.idsAndFiles = files.associateBy { it.id }
            this.suggestedDocs = suggestedDocs(idsAndFiles).getOrElse { err ->
                return Err(err)
            }
            refreshChildren()
        }
    }

    fun intoFile(newParent: File) {
        parent = newParent
        refreshChildren()
        fileDir.add(newParent)
    }

    fun intoParent() {
        parent = idsAndFiles[parent.parent]!!
        refreshChildren()
        fileDir.removeLast()
    }

    fun refreshChildren() {
        children = idsAndFiles.values.filter { it.parent == parent.id && it.id != it.parent }
        sortChildren()
    }

    private fun sortChildren() {
        children = sortFiles(children)
    }
}
