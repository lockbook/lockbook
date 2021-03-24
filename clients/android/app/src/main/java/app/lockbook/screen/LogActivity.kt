package app.lockbook.screen

import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.R
import kotlinx.android.synthetic.main.activity_debug.*
import java.io.File

class LogActivity : AppCompatActivity() {

    companion object {
        private const val LOG_FILE_NAME = "lockbook.log"
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_debug)

        getDebugContent()
    }

    private fun getDebugContent() {
        debug_text.text = File("$filesDir/$LOG_FILE_NAME").readText()
    }
}
