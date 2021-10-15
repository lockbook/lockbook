package app.lockbook.util

interface FilesFragment {
    fun refreshFiles()
    fun unselectFiles()
    fun onNewFileCreated(newDocument: ClientFileMetadata?)
    fun onBackPressed(): Boolean
}
