package app.lockbook.util

interface ListFilesClickInterface {
    fun onItemClick(position: Int, isSelecting: Boolean, selection: List<Boolean>)
    fun onLongClick(position: Int, selection: List<Boolean>) {}
}

interface RegularClickInterface {
    fun onItemClick(position: Int) {}
}
