package app.lockbook.util

interface FilesFragment {
    fun refreshFiles()
    fun unselectFiles()
    fun onNewFileCreated(newDocument: File?)
    fun onBackPressed(): Boolean
    fun syncBasedOnPreferences()
}
