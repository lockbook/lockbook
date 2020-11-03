package app.lockbook.util

interface ClickInterface {
    fun onItemClick(position: Int, isSelecting: Boolean, selection: Boolean)
    fun onLongClick(position: Int, selection: Boolean) {}
}
