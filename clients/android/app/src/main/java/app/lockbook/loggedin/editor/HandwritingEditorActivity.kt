package app.lockbook.loggedin.editor

import android.graphics.SurfaceTexture
import android.os.Bundle
import android.os.Handler
import android.view.SurfaceHolder
import android.view.TextureView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.ViewModelProvider
import app.lockbook.R
import app.lockbook.utils.LockbookDrawable
import app.lockbook.utils.TEXT_EDITOR_BACKGROUND_SAVE_PERIOD
import com.beust.klaxon.Klaxon
import kotlinx.android.synthetic.main.activity_handwriting_editor.*
import timber.log.Timber
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
            Timber.e(contents)
            handwriting_editor.lockBookDrawable = Klaxon().parse(contents)!!
            handwriting_editor.holder.addCallback(object: SurfaceHolder.Callback {
                override fun surfaceCreated(holder: SurfaceHolder?) {
                    handwriting_editor.setUpBitmapCanvas()
                    handwriting_editor.drawLockbookDrawable()
                }

                override fun surfaceChanged(
                    holder: SurfaceHolder?,
                    format: Int,
                    width: Int,
                    height: Int
                ) {}

                override fun surfaceDestroyed(holder: SurfaceHolder?) {}
            })
        } else {
            handwriting_editor.holder.addCallback(object: SurfaceHolder.Callback {
                override fun surfaceCreated(holder: SurfaceHolder?) {
                    handwriting_editor.setUpBitmapCanvas()
                }

                override fun surfaceChanged(
                    holder: SurfaceHolder?,
                    format: Int,
                    width: Int,
                    height: Int
                ) {}

                override fun surfaceDestroyed(holder: SurfaceHolder?) {}
            })

        }

        startBackgroundSave()
    }

    private fun startBackgroundSave() {
        timer.schedule(
            object : TimerTask() {
                override fun run() {
                    handler.post {
//                        handwritingEditorViewModel.savePath(handwriting_editor.lockBookDrawable)
                    }
                }
            },
            1000,
            TEXT_EDITOR_BACKGROUND_SAVE_PERIOD
        )
    }

    override fun onDestroy() {
        timer.cancel()
        handwritingEditorViewModel.savePath(handwriting_editor.lockBookDrawable)
        super.onDestroy()
    }

    private fun errorHasOccurred(errorText: String) {
        finish()
        Toast.makeText(applicationContext, errorText, Toast.LENGTH_LONG).show()
    }
}
