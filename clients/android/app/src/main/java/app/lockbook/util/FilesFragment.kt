package app.lockbook.util

import net.lockbook.File

interface FilesFragment {
    fun refreshFiles()
    fun unselectFiles()
    fun onNewFileCreated(newDocument: File?)
    fun onBackPressed(): Boolean
    fun sync(usePreferences: Boolean = true)
}
