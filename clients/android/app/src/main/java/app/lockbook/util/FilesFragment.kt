package app.lockbook.util

interface FilesFragment {
    fun refreshFiles()
    fun unselectFiles()
    fun onNewFileCreated(newDocument: DecryptedFileMetadata?)
    fun onBackPressed(): Boolean
    fun syncBasedOnPreferences()
}
