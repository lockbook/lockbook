package app.lockbook.util

interface ListFilesClickInterface {
    fun onItemClick(position: Int, newSelectedFiles: List<File>)
    fun onLongClick(position: Int, newSelectedFiles: List<File>) {}
}

interface RegularClickInterface {
    fun onItemClick(position: Int) {}
}
