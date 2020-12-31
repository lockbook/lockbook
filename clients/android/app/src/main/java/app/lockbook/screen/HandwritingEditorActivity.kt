package app.lockbook.screen

import android.os.Bundle
import android.os.Handler
import android.view.SurfaceHolder
import android.view.View
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.res.ResourcesCompat
import androidx.lifecycle.ViewModelProvider
import app.lockbook.App
import app.lockbook.R
import app.lockbook.model.HandwritingEditorViewModel
import app.lockbook.modelfactory.HandwritingEditorViewModelFactory
import app.lockbook.ui.HandwritingEditorView
import app.lockbook.util.*
import app.lockbook.util.Messages.UNEXPECTED_ERROR
import com.google.android.material.button.MaterialButton
import com.google.android.material.snackbar.Snackbar
import kotlinx.android.synthetic.main.activity_handwriting_editor.*
import java.util.*

class HandwritingEditorActivity : AppCompatActivity() {
    private lateinit var handwritingEditorViewModel: HandwritingEditorViewModel
    private val surfaceViewReadyCallback = object : SurfaceHolder.Callback {
        override fun surfaceCreated(holder: SurfaceHolder?) {
            addDrawingToView()
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
    }

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

        startDrawing(id)

        handwritingEditorViewModel.drawableReady.observe(
            this
        ) {
            handwriting_editor.holder.addCallback(surfaceViewReadyCallback)

            if (!handwriting_editor.holder.isCreating) {
                addDrawingToView()
            }
        }

        startBackgroundSave()
        setUpHandwritingToolbar()
    }

    private fun addDrawingToView() {
        handwriting_editor.isTouchable = true
        handwriting_editor_progress_bar.visibility = View.GONE
        handwriting_editor.initializeWithDrawing(handwritingEditorViewModel.lockBookDrawable)
    }

    private fun startDrawing(id: String) {
        handwriting_editor_progress_bar.visibility = View.VISIBLE

        if (handwritingEditorViewModel.lockBookDrawable == null) {
            handwritingEditorViewModel.getDrawing(id)
        } else {
            handwriting_editor.holder.addCallback(surfaceViewReadyCallback)
        }
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
        drawing_color_white.setOnClickListener {
            newStylusColorSelected(drawing_color_white, android.R.color.white)
        }

        drawing_color_blue.setOnClickListener {
            newStylusColorSelected(drawing_color_blue, android.R.color.holo_blue_light)
        }

        drawing_color_green.setOnClickListener {
            newStylusColorSelected(drawing_color_green, android.R.color.holo_green_light)
        }

        drawing_color_orange.setOnClickListener {
            newStylusColorSelected(drawing_color_orange, android.R.color.holo_orange_light)
        }

        drawing_color_purple.setOnClickListener {
            newStylusColorSelected(drawing_color_purple, android.R.color.holo_purple)
        }

        drawing_color_red.setOnClickListener {
            newStylusColorSelected(drawing_color_red, android.R.color.holo_red_light)
        }

        drawing_erase.setOnClickListener {
            drawing_erase.setBackgroundResource(R.drawable.item_border)
            drawing_pen.setBackgroundResource(0)
            handwriting_editor.isErasing = true
        }

        drawing_pen.setOnClickListener {
            drawing_pen.setBackgroundResource(R.drawable.item_border)
            drawing_erase.setBackgroundResource(0)
            handwriting_editor.isErasing = false
        }

        handwriting_editor_pen_small.setOnClickListener {
            handwriting_editor_pen_small.setBackgroundResource(R.drawable.item_border)
            handwriting_editor_pen_medium.setBackgroundResource(0)
            handwriting_editor_pen_large.setBackgroundResource(0)
            handwriting_editor.setPenSize(HandwritingEditorView.PenSize.SMALL)

        }

        handwriting_editor_pen_medium.setOnClickListener {
            handwriting_editor_pen_medium.setBackgroundResource(R.drawable.item_border)
            handwriting_editor_pen_small.setBackgroundResource(0)
            handwriting_editor_pen_large.setBackgroundResource(0)
            handwriting_editor.setPenSize(HandwritingEditorView.PenSize.MEDIUM)
        }

        handwriting_editor_pen_large.setOnClickListener {
            handwriting_editor_pen_large.setBackgroundResource(R.drawable.item_border)
            handwriting_editor_pen_small.setBackgroundResource(0)
            handwriting_editor_pen_medium.setBackgroundResource(0)
            handwriting_editor.setPenSize(HandwritingEditorView.PenSize.LARGE)
        }
    }

    private fun newStylusColorSelected(button: MaterialButton, colorId: Int) {
        val color = ResourcesCompat.getColor(
            App.instance.resources,
            colorId,
            App.instance.theme
        )
        handwriting_editor.setColor(color)
        drawing_color_white.strokeWidth = 0
        drawing_color_blue.strokeWidth = 0
        drawing_color_green.strokeWidth = 0
        drawing_color_orange.strokeWidth = 0
        drawing_color_purple.strokeWidth = 0
        drawing_color_red.strokeWidth = 0
        button.strokeWidth = 2
        button.setStrokeColorResource(R.color.blue)
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
                                            event.stroke.points.toMutableList()
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
        AlertDialog.Builder(this, R.style.Main_Dialog)
            .setTitle(UNEXPECTED_ERROR)
            .setMessage(error)
            .setOnCancelListener {
                finish()
            }
            .show()
    }
}
