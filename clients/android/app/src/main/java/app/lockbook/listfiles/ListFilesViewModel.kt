package app.lockbook.listfiles

import androidx.lifecycle.ViewModel
import app.lockbook.ClientFileMetadata
import app.lockbook.core.getRoot
import com.beust.klaxon.Klaxon
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import java.io.File

class ListFilesViewModel(var path: File): ViewModel() {

    private var viewModelJob = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + viewModelJob)
    val filesFolders: ClientFileMetadata? = Klaxon().parse(getRoot(path.absolutePath))

}