package app.lockbook.loggedin.editor

import android.os.Bundle
import android.os.Handler
import android.view.SurfaceHolder
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.ViewModelProvider
import app.lockbook.R
import app.lockbook.utils.*
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

        if (id == null) {
            errorHasOccurred("Unable to retrieve id.")
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

        if (startUpDrawing(id)) return

        startBackgroundSave()
    }

    private fun startUpDrawing(id: String): Boolean {
        val contents = handwritingEditorViewModel.handleReadDocument(id)

        if (contents != null && contents.isNotEmpty()) {
            val lockbookDrawable = if(handwritingEditorViewModel.lockBookDrawable == null) {

                Klaxon().parse<Drawing>(contents)
            } else {
                handwritingEditorViewModel.lockBookDrawable
            }

            if (lockbookDrawable == null) {
                errorHasOccurred("Unable to load this drawing.")
                return true
            }

            handwriting_editor.lockBookDrawable = lockbookDrawable
            handwriting_editor.holder.addCallback(object : SurfaceHolder.Callback {
                override fun surfaceCreated(holder: SurfaceHolder?) {
                    handwriting_editor.setUpBitmapDrawable()
                    handwriting_editor.drawLockbookDrawable()
                }

                override fun surfaceChanged(
                    holder: SurfaceHolder?,
                    format: Int,
                    width: Int,
                    height: Int
                ) {
                }

                override fun surfaceDestroyed(holder: SurfaceHolder?) {
                }
            })


        } else {
            handwriting_editor.holder.addCallback(object : SurfaceHolder.Callback {
                override fun surfaceCreated(holder: SurfaceHolder?) {
                    handwriting_editor.setUpBitmapDrawable()
                }

                override fun surfaceChanged(
                    holder: SurfaceHolder?,
                    format: Int,
                    width: Int,
                    height: Int
                ) {
                }

                override fun surfaceDestroyed(holder: SurfaceHolder?) {
                }
            })
        }

        return false
    }

    private fun startBackgroundSave() { // could this crash if the threads take too long to finish and they keep saving?!
        timer.schedule(
            object : TimerTask() {
                override fun run() {
                    handler.post {
                        handwritingEditorViewModel.savePath(
                            Drawing(
                                Page(
                                    Transformation(
                                        Point(
                                            handwriting_editor.lockBookDrawable.page.transformation.translation.x,
                                            handwriting_editor.lockBookDrawable.page.transformation.translation.y
                                        ),
                                        handwriting_editor.lockBookDrawable.page.transformation.scale,
                                        handwriting_editor.lockBookDrawable.page.transformation.rotation
                                    )
                                ),
                                handwriting_editor.lockBookDrawable.events.map { event ->
                                    Event(
                                        if (event.stroke == null) null else Stroke(
                                            event.stroke.color,
                                            event.stroke.points.map { point ->
                                                PressurePoint(
                                                    point.x,
                                                    point.y,
                                                    point.pressure
                                                )
                                            }.toMutableList())
                                    )
                                }.toMutableList()
                            )
                        )
                    }
                }
            },
            1000,
            TEXT_EDITOR_BACKGROUND_SAVE_PERIOD
        )
    }

    override fun onDestroy() {
        timer.cancel()
        handwritingEditorViewModel.lockBookDrawable = handwriting_editor.lockBookDrawable
        handwritingEditorViewModel.savePath(handwriting_editor.lockBookDrawable)
        super.onDestroy()
    }

    private fun errorHasOccurred(errorText: String) {
        finish()
        Toast.makeText(applicationContext, errorText, Toast.LENGTH_LONG).show()
    }
}
