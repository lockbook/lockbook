package app.lockbook.screen

import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.R
import com.google.android.material.textview.MaterialTextView
import java.io.File

class LogActivity : AppCompatActivity() {

    companion object {
        const val LOG_FILE_NAME = "lockbook.log"
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_debug)

        getDebugContent()
    }

    private fun getDebugContent() {
        this.findViewById<MaterialTextView>(R.id.debug_text).text = File("$filesDir/$LOG_FILE_NAME").readText()
    }
}
