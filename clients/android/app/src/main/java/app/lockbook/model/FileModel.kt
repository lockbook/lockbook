package app.lockbook.model

import net.lockbook.File
import net.lockbook.Lb
import net.lockbook.LbError

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
        fun createAtRoot(): FileModel {
            val root = Lb.getRoot()

            val fileModel = FileModel(
                root,
                root,
                emptyMap(),
                listOf(),
                listOf(),
                mutableListOf(root),
            )
            fileModel.refreshFiles()

            return fileModel
        }

        fun sortFiles(files: List<File>): List<File> = files.sortedWith(compareBy<File> { it.type }.thenBy { it.name })
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

    fun refreshFiles() {
        idsAndFiles = (Lb.listMetadatas()).associateBy { it.id }
        suggestedDocs = Lb.suggestedDocs().mapNotNull { idsAndFiles[it] }
        refreshChildren()
    }

    fun intoFile(newParent: File) {
        if (newParent.parent == root.id && fileDir.size > 1) {
            fileDir.clear()
            fileDir.add(root)
            fileDir.add(newParent)
        } else {
            try {
                var curr: File? = newParent
                val temp: MutableList<File> = mutableListOf()

                while (curr != null && curr.id != parent.id && !curr.isRoot) {
                    temp.add(curr)
                    curr = idsAndFiles[curr.parent]
                }
                temp.reverse()
                fileDir.addAll(temp)
            } catch (err: LbError) {
                println(err)
            }
        }

        parent = newParent
        refreshChildren()
    }

    fun intoParent() {
        parent = idsAndFiles[parent.parent]!!
        refreshChildren()
        fileDir.removeAt(fileDir.lastIndex)
    }

    fun verifyOpenFile(id: String): Boolean {
        val file = idsAndFiles[id] ?: return false

        if (file.parent == root.id && fileDir.size > 1) {
            refreshFiles()

            fileDir.clear()
            fileDir.add(root)

            parent = root
            refreshChildren()

            return true
        } else {
            return false
        }
    }

    private fun refreshChildren() {
        children = sortFiles(idsAndFiles.values.filter { it.parent == parent.id && it.id != it.parent })
    }
}
