package app.lockbook.loggedin.newfilefolder

import android.app.Activity
import android.os.Bundle
import android.util.Log
import androidx.databinding.DataBindingUtil
import app.lockbook.R
import app.lockbook.core.createFileFolder
import app.lockbook.core.getChildren
import app.lockbook.databinding.ActivityNewFileFolderBinding
import app.lockbook.utils.FileType
import com.beust.klaxon.Klaxon
import kotlinx.android.synthetic.main.activity_new_file_folder.*
import kotlinx.android.synthetic.main.activity_new_file_folder.name_text
import kotlinx.coroutines.*

class NewFileFolderActivity : Activity() {

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val binding: ActivityNewFileFolderBinding = DataBindingUtil.setContentView(
            this,
            R.layout.activity_new_file_folder
        )

        binding.newFileFolderActivity = this
    }

    fun createFileFolder() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                val fileType = if (file_radio_button.isSelected) {
                    Klaxon().toJsonString(FileType.Document)
                } else {
                    Klaxon().toJsonString(FileType.Folder.toString())
                }

                val path = intent.getStringExtra("path")
                val parentUuid = intent.getStringExtra("parentUuid")

                createFileFolder(
                    path,
                    parentUuid,
                    fileType,
                    name_text.text.toString()
                )

                withContext(Dispatchers.Main) {
                    finish()
                }
            }
        }
    }
}