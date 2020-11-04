package app.lockbook.util

interface ListFilesClickInterface {
    fun onItemClick(position: Int, isSelecting: Boolean, selection: Boolean)
    fun onLongClick(position: Int, selection: Boolean) {}
}

interface RegularClickInterface {
    fun onItemClick(position: Int) {}
}
