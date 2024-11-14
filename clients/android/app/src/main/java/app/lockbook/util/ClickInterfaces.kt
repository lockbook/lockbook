package app.lockbook.util

import net.lockbook.File

interface ListFilesClickInterface {
    fun onItemClick(position: Int, newSelectedFiles: List<File>)
    fun onLongClick(position: Int, newSelectedFiles: List<File>) {}
}

interface RegularClickInterface {
    fun onItemClick(position: Int) {}
}
