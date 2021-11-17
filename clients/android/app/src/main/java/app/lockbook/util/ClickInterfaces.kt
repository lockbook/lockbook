package app.lockbook.util

interface ListFilesClickInterface {
    fun onItemClick(position: Int, newSelectedFiles: List<DecryptedFileMetadata>)
    fun onLongClick(position: Int, newSelectedFiles: List<DecryptedFileMetadata>) {}
}

interface RegularClickInterface {
    fun onItemClick(position: Int) {}
}
