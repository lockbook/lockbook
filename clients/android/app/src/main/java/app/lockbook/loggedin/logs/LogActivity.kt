package app.lockbook.loggedin.logs

import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.R
import app.lockbook.utils.LOG_FILE_NAME
import kotlinx.android.synthetic.main.activity_debug.*
import kotlinx.android.synthetic.main.activity_debug.view.*
import kotlinx.coroutines.*
import java.io.File

class LogActivity : AppCompatActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_debug)

        getDebugContent()
    }

    private fun getDebugContent() {
        debug_text.text = File("$filesDir/$LOG_FILE_NAME").readText()
    }
}
