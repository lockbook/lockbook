package app.lockbook.util

interface ListFilesClickInterface {
    fun onItemClick(position: Int, newSelectedFiles: List<ClientFileMetadata>)
    fun onLongClick(position: Int, newSelectedFiles: List<ClientFileMetadata>) {}
}

interface RegularClickInterface {
    fun onItemClick(position: Int) {}
}
