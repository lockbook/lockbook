package app.lockbook.util

import net.lockbook.File

interface FilesFragment {
    fun reloadFiles()

    fun unselectFiles()

    fun onNewFileCreated(newDocument: File?)

    fun onBackPressed(): Boolean
}
