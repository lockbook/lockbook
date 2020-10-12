package app.lockbook.loggedin.editor

import android.os.Bundle
import android.os.Handler
import android.view.SurfaceHolder
import android.view.View
import android.widget.AdapterView
import android.widget.ArrayAdapter
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.ViewModelProvider
import app.lockbook.R
import app.lockbook.utils.*
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
        val name = intent.getStringExtra("name")

        if (id == null) {
            errorHasOccurred("Unable to retrieve id.")
            finish()
            return
        }

        if (name == null) {
            errorHasOccurred("Unable to retrieve name.")
            finish()
            return
        }
        handwriting_editor_drawing_name.text = name

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

        if (!startUpDrawingIfAble(id)) {
            finish()
            return
        }

        startBackgroundSave()
        setUpHandwritingToolbar()
    }

    private fun startUpDrawingIfAble(id: String): Boolean {
        val contents = handwritingEditorViewModel.handleReadDocument(id)
        Timber.e(contents)

        if (contents != null && contents.isNotEmpty()) {
            if (handwritingEditorViewModel.lockBookDrawable == null) {
                handwritingEditorViewModel.lockBookDrawable = Klaxon().parse<Drawing>(contents)
            }

            if (handwritingEditorViewModel.lockBookDrawable == null) {
                errorHasOccurred("Unable to load this drawing.")
                return false
            }
        }

        handwriting_editor.holder.addCallback(object : SurfaceHolder.Callback {
            override fun surfaceCreated(holder: SurfaceHolder?) {
                handwriting_editor.setUpDrawing(handwritingEditorViewModel.lockBookDrawable)
            }

            override fun surfaceChanged(
                holder: SurfaceHolder?,
                format: Int,
                width: Int,
                height: Int
            ) {
            }

            override fun surfaceDestroyed(holder: SurfaceHolder?) {
                endHandwritingThread()
            }
        })

        return true
    }

    override fun onPause() {
        super.onPause()
        endHandwritingThread()
    }

    override fun onRestart() {
        super.onRestart()
        handwriting_editor.restartThread()
    }

    private fun endHandwritingThread() {
        handwriting_editor.endThread()
    }

    private fun setUpHandwritingToolbar() {
        ArrayAdapter.createFromResource(
            this,
            R.array.handwriting_editor_pallete_colors,
            android.R.layout.simple_spinner_item
        ).also { adapter ->
            adapter.setDropDownViewResource(android.R.layout.simple_spinner_dropdown_item)
            handwriting_editor_pallete_spinner.adapter = adapter
        }

        handwriting_editor_pallete_spinner.onItemSelectedListener =
            object : AdapterView.OnItemSelectedListener {
                override fun onItemSelected(
                    parent: AdapterView<*>?,
                    view: View?,
                    position: Int,
                    id: Long
                ) {
                    handwriting_editor.setColor(parent?.getItemAtPosition(position).toString())
                }

                override fun onNothingSelected(parent: AdapterView<*>?) {}
            }
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
                                            handwriting_editor.lockbookDrawable.page.transformation.translation.x,
                                            handwriting_editor.lockbookDrawable.page.transformation.translation.y
                                        ),
                                        handwriting_editor.lockbookDrawable.page.transformation.scale,
                                        handwriting_editor.lockbookDrawable.page.transformation.scaleFocus,
                                        handwriting_editor.lockbookDrawable.page.transformation.rotation
                                    )
                                ),
                                handwriting_editor.lockbookDrawable.events.map { event ->
                                    Event(
                                        if (event.stroke == null) null else Stroke(
                                            event.stroke.color,
                                            event.stroke.points.map { point ->
                                                PressurePoint(
                                                    point.x,
                                                    point.y,
                                                    point.pressure
                                                )
                                            }.toMutableList()
                                        )
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
        handwritingEditorViewModel.lockBookDrawable = handwriting_editor.lockbookDrawable
        handwritingEditorViewModel.savePath(handwriting_editor.lockbookDrawable)
        super.onDestroy()
    }

    private fun errorHasOccurred(errorText: String) {
        finish()
        Toast.makeText(applicationContext, errorText, Toast.LENGTH_LONG).show()
    }
}
