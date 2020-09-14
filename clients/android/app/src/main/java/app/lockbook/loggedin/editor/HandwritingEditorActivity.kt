package app.lockbook.loggedin.editor

import android.os.Bundle
import android.os.Handler
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.ViewModelProvider
import app.lockbook.R
import app.lockbook.utils.Path
import app.lockbook.utils.TEXT_EDITOR_BACKGROUND_SAVE_PERIOD
import com.beust.klaxon.Klaxon
import kotlinx.android.synthetic.main.activity_handwriting_editor.*
import java.util.*

class HandwritingEditorActivity : AppCompatActivity() {
    private lateinit var handwritingEditorViewModel: HandwritingEditorViewModel
    private var timer: Timer = Timer()
    private val handler = Handler()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_handwriting_editor)

        val id = intent.getStringExtra("id")
        val contents = intent.getStringExtra("contents")

        if (id == null) {
            errorHasOccurred("Unable to retrieve id.")
            finish()
            return
        }
        if (contents == null) {
            errorHasOccurred("Unable to retrieve contents.")
            finish()
            return
        }

        handwritingEditorViewModel =
            ViewModelProvider(
                this,
                HandwritingEditorViewModelFactory(application, id)
            ).get(HandwritingEditorViewModel::class.java)

        handwritingEditorViewModel.errorHasOccurred.observe(
            this,
            { errorHasOccurred ->
                errorHasOccurred(errorHasOccurred)
            }
        )

        if (contents.isNotEmpty()) {
            val paths = Klaxon().parseArray<Path>(contents)
            if (paths != null) {
                handwriting_editor.drawnPaths = paths.toMutableList()
                handwriting_editor.reOpened = true
            } else {
                errorHasOccurred("Unable to parse old view together: $contents")
                finish()
                return
            }
        }

        startBackgroundSave()
    }

    override fun onStart() {
        super.onStart()
        handwriting_editor.invalidate()
    }

    private fun startBackgroundSave() {
        timer.schedule(
            object : TimerTask() {
                override fun run() {
                    handler.post {
                        handwritingEditorViewModel.savePath(handwriting_editor.drawnPaths.toList())
                    }
                }
            },
            1000,
            TEXT_EDITOR_BACKGROUND_SAVE_PERIOD
        )
    }

    override fun onDestroy() {
        timer.cancel()
        handwritingEditorViewModel.savePath(handwriting_editor.drawnPaths.toList())
        super.onDestroy()
    }

    private fun errorHasOccurred(errorText: String) {
        finish()
        Toast.makeText(applicationContext, errorText, Toast.LENGTH_LONG).show()
    }
}
