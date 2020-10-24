package app.lockbook.loggedin.editor

import android.os.Bundle
import android.os.Handler
import android.view.SurfaceHolder
import android.view.View
import android.widget.AdapterView
import android.widget.ArrayAdapter
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.ViewModelProvider
import app.lockbook.R
import app.lockbook.utils.*
import app.lockbook.utils.Messages.UNEXPECTED_ERROR
import com.beust.klaxon.Klaxon
import com.google.android.material.snackbar.Snackbar
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
            this
        ) { errorText ->
            errorHasOccurred(errorText)
        }

        handwritingEditorViewModel.unexpectedErrorHasOccurred.observe(
            this
        ) { errorText ->
            unexpectedErrorHasOccurred(errorText)
        }

        if (!startUpDrawingIfAble(id)) {
            finish()
            return
        }

        startBackgroundSave()
        setUpHandwritingToolbar()
    }

    private fun startUpDrawingIfAble(id: String): Boolean {
        val priorContents = handwritingEditorViewModel.handleReadDocument(id)

        if (priorContents != null && priorContents.isNotEmpty()) {
            if (handwritingEditorViewModel.lockBookDrawable == null) {
                handwritingEditorViewModel.lockBookDrawable = Klaxon().parse<Drawing>(priorContents)
            }

            if (handwritingEditorViewModel.lockBookDrawable == null) {
                errorHasOccurred("Unable to load this drawing.")
                return false
            }
        }

        handwriting_editor.holder.addCallback(object : SurfaceHolder.Callback {
            override fun surfaceCreated(holder: SurfaceHolder?) {
                handwriting_editor.initializeWithDrawing(handwritingEditorViewModel.lockBookDrawable)
            }

            override fun surfaceChanged(
                holder: SurfaceHolder?,
                format: Int,
                width: Int,
                height: Int
            ) {
            }

            override fun surfaceDestroyed(holder: SurfaceHolder?) {
                handwriting_editor.endThread()
            }
        })

        return true
    }

    override fun onRestart() {
        super.onRestart()
        handwriting_editor.restartThread()
    }

    override fun onPause() {
        super.onPause()
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
                                            handwriting_editor.drawingModel.currentView.transformation.translation.x,
                                            handwriting_editor.drawingModel.currentView.transformation.translation.y
                                        ),
                                        handwriting_editor.drawingModel.currentView.transformation.scale,
                                    )
                                ),
                                handwriting_editor.drawingModel.events.map { event ->
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
        handwritingEditorViewModel.lockBookDrawable = handwriting_editor.drawingModel
        handwritingEditorViewModel.savePath(handwriting_editor.drawingModel)
        super.onDestroy()
    }

    private fun errorHasOccurred(error: String) {
        Snackbar.make(handwriting_editor_layout, error, Snackbar.LENGTH_SHORT)
            .addCallback(object : Snackbar.Callback() {
                override fun onDismissed(transientBottomBar: Snackbar?, event: Int) {
                    super.onDismissed(transientBottomBar, event)
                    finish()
                }
            }).show()
    }

    private fun unexpectedErrorHasOccurred(error: String) {
        AlertDialog.Builder(this, R.style.DarkBlue_Dialog)
            .setTitle(UNEXPECTED_ERROR)
            .setMessage(error)
            .setOnCancelListener {
                finish()
            }
            .show()
    }
}
