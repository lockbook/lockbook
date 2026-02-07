package app.lockbook.model

import net.lockbook.File
import net.lockbook.Lb

class FileModel(
    val root: File,
    var parent: File,
    var idsAndFiles: Map<String, File>,
    var children: List<File>,
    var suggestedDocs: List<File>,
) {

    companion object {
        // Returns Ok(null) if there is no root
        fun createAtRoot(): FileModel {
            val root = Lb.getRoot()

            val fileModel = FileModel(
                root,
                root,
                emptyMap(),
                listOf(),
                listOf(),
            )
            fileModel.refreshFiles()

            return fileModel
        }

        fun sortFiles(files: List<File>): List<File> = files.sortedWith(compareBy<File> { it.type }.thenBy { it.name })
    }

    fun refreshChildrenAtAncestor(newParent: File) {
        parent = newParent
        refreshChildren()
    }

    fun isAtRoot(): Boolean = parent.id == parent.parent

    fun refreshFiles() {
        idsAndFiles = (Lb.listMetadatas() + Lb.getPendingShareFiles()).associateBy { it.id }
        suggestedDocs = Lb.suggestedDocs().mapNotNull { idsAndFiles[it] }
        refreshChildren()
    }

    fun getFileDir(): MutableList<File> {
        var curr: File = parent
        val temp: MutableList<File> = mutableListOf()
        while (true) {
            temp.add(curr)

            if (curr.isRoot) {
                break
            }

            curr = idsAndFiles[curr.parent] ?: break
        }
        temp.reverse()
        return temp
    }
    fun enterFolder(newParent: File) {
        parent = newParent
        refreshChildren()
    }

    fun intoParent() {
        parent = idsAndFiles[parent.parent]!!
        refreshChildren()
    }

    private fun refreshChildren() {
        children = sortFiles(idsAndFiles.values.filter { it.parent == parent.id && it.id != it.parent })
    }
}
