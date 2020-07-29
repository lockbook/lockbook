package app.lockbook.loggedin.newfile

import android.app.Activity
import android.os.Bundle
import app.lockbook.R
import app.lockbook.utils.FileType
import com.beust.klaxon.Klaxon
import kotlinx.android.synthetic.main.activity_new_file.*
import kotlinx.android.synthetic.main.activity_new_file.name_text
import kotlinx.coroutines.*

class NewFileActivity : Activity() {

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_new_file)

        new_file_create_file.setOnClickListener {
            createFile()
        }
    }

    fun createFile() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                val json = Klaxon()
                val fileType = if (file_radio_button.isChecked) {
                    json.toJsonString(FileType.Document)
                } else {
                    json.toJsonString(FileType.Folder)
                }

                intent.putExtra("fileType", fileType)
                intent.putExtra("name", name_text.text.toString())

                setResult(RESULT_OK, intent)

                withContext(Dispatchers.Main) {
                    finish()
                }
            }
        }
    }
}
