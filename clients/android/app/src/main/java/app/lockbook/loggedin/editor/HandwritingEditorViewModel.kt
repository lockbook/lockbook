package app.lockbook.loggedin.editor

import android.app.Application
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Path
import android.os.Handler
import androidx.core.graphics.applyCanvas
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import app.lockbook.utils.*
import com.caverock.androidsvg.SVG
import com.github.michaelbull.result.Err
import kotlinx.android.synthetic.main.activity_debug.*
import kotlinx.android.synthetic.main.activity_text_editor.*
import timber.log.Timber
import java.util.*

class HandwritingEditorViewModel(
    application: Application,
    private val id: String
) : AndroidViewModel(application) {

    private val config = Config(getApplication<Application>().filesDir.absolutePath)
    private val _errorHasOccurred = MutableLiveData<String>()

    val errorHasOccurred: LiveData<String>
        get() = _errorHasOccurred

    fun saveSVG(svg: String) {
        Timber.e("SMAIL1: $svg")
        val writeToDocumentResult = CoreModel.writeContentToDocument(config, id, svg.removeSuffix("</svg>").removePrefix("<svg>"))
        if (writeToDocumentResult is Err) {
            when (val error = writeToDocumentResult.error) {
                is WriteToDocumentError.FolderTreatedAsDocument -> {
                    _errorHasOccurred.postValue("Error! Folder is treated as document!")
                }
                is WriteToDocumentError.FileDoesNotExist -> {
                    _errorHasOccurred.postValue("Error! File does not exist!")
                }
                is WriteToDocumentError.NoAccount -> {
                    _errorHasOccurred.postValue("Error! No account!")
                }
                is WriteToDocumentError.UnexpectedError -> {
                    Timber.e("Unable to write document changes: ${error.error}")
                    _errorHasOccurred.postValue(
                        Messages.UNEXPECTED_ERROR_OCCURRED
                    )
                }
                else -> {
                    Timber.e("WriteToDocumentError not matched: ${error::class.simpleName}.")
                    _errorHasOccurred.postValue(Messages.UNEXPECTED_ERROR_OCCURRED)
                }
            }
        }
    }

}