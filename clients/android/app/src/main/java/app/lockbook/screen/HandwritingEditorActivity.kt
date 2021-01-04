package app.lockbook.screen

import android.annotation.SuppressLint
import android.os.Bundle
import android.os.Handler
import android.view.GestureDetector
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.View
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.ViewModelProvider
import app.lockbook.R
import app.lockbook.model.HandwritingEditorViewModel
import app.lockbook.modelfactory.HandwritingEditorViewModelFactory
import app.lockbook.ui.HandwritingEditorView
import app.lockbook.util.*
import app.lockbook.util.Messages.UNEXPECTED_CLIENT_ERROR
import app.lockbook.util.Messages.UNEXPECTED_ERROR
import com.google.android.material.snackbar.Snackbar
import kotlinx.android.synthetic.main.activity_handwriting_editor.*
import timber.log.Timber
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

    private var autoSaveTimer = Timer()
    private val handler = Handler()
    private lateinit var gestureDetector: GestureDetector

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

        handwritingEditorViewModel.selectNewColor.observe(
            this
        ) { colors ->
            selectNewColor(colors.first, colors.second)
        }

        handwritingEditorViewModel.setToolsVisibility.observe(
            this
        ) { newVisibility ->
            changeToolsVisibility(newVisibility)
        }

        handwritingEditorViewModel.selectNewTool.observe(
            this
        ) { tools ->
            selectNewTool(tools.second)
        }

        handwritingEditorViewModel.selectedNewPenSize.observe(
            this
        ) { penSizes ->
            selectedNewPenSize(penSizes.first, penSizes.second)
        }

        startBackgroundSave()
        setUpToolbarListeners()
        setUpToolbarDefaults()
    }

    override fun onRestart() {
        super.onRestart()
        handwriting_editor.restartThread()
    }

    override fun onPause() {
        super.onPause()
        handwriting_editor.endThread()
    }

    private fun selectNewColor(oldColor: Int?, newColor: Int) {
        if (oldColor != null) {
            val previousButton = when (oldColor) {
                android.R.color.white -> drawing_color_white
                android.R.color.holo_blue_light -> drawing_color_blue
                android.R.color.holo_green_light -> drawing_color_green
                android.R.color.holo_orange_light -> drawing_color_orange
                android.R.color.holo_purple -> drawing_color_purple
                android.R.color.holo_red_light -> drawing_color_red
                else -> {
                    errorHasOccurred(UNEXPECTED_CLIENT_ERROR)
                    Timber.e("The previously selected color from the toolbar is not handled: $oldColor")
                    return
                }
            }.exhaustive

            previousButton.strokeWidth = 0
        }

        val newButton = when (newColor) {
            android.R.color.white -> drawing_color_white
            android.R.color.holo_blue_light -> drawing_color_blue
            android.R.color.holo_green_light -> drawing_color_green
            android.R.color.holo_orange_light -> drawing_color_orange
            android.R.color.holo_purple -> drawing_color_purple
            android.R.color.holo_red_light -> drawing_color_red
            else -> {
                errorHasOccurred(UNEXPECTED_CLIENT_ERROR)
                Timber.e("The newly selected color from the toolbar is not handled: $newColor")
                return
            }
        }.exhaustive

        newButton.strokeWidth = 4
        handwriting_editor.setColor(newColor)
    }

    private fun selectNewTool(newTool: HandwritingEditorView.Tool) {
        if (newTool == HandwritingEditorView.Tool.PEN) {
            drawing_pen.setImageResource(R.drawable.ic_pencil_filled)
            drawing_erase.setImageResource(R.drawable.ic_eraser_outline)
            handwriting_editor.isErasing = false
        } else {
            drawing_erase.setImageResource(R.drawable.ic_eraser_filled)
            drawing_pen.setImageResource(R.drawable.ic_pencil_outline)
            handwriting_editor.isErasing = true
        }
    }

    private fun selectedNewPenSize(
        oldPenSize: HandwritingEditorView.PenSize?,
        newPenSize: HandwritingEditorView.PenSize
    ) {
        if (oldPenSize != null) {
            val previousButton = when (oldPenSize) {
                HandwritingEditorView.PenSize.SMALL -> handwriting_editor_pen_small
                HandwritingEditorView.PenSize.MEDIUM -> handwriting_editor_pen_medium
                HandwritingEditorView.PenSize.LARGE -> handwriting_editor_pen_large
            }.exhaustive

            previousButton.setBackgroundResource(0)
        }

        val newButton = when (newPenSize) {
            HandwritingEditorView.PenSize.SMALL -> handwriting_editor_pen_small
            HandwritingEditorView.PenSize.MEDIUM -> handwriting_editor_pen_medium
            HandwritingEditorView.PenSize.LARGE -> handwriting_editor_pen_large
        }.exhaustive

        newButton.setBackgroundResource(R.drawable.item_border)
        handwriting_editor.setPenSize(newPenSize)
    }

    private fun setUpToolbarDefaults() {
        val colorButtons = listOf(drawing_color_white, drawing_color_blue, drawing_color_green, drawing_color_orange, drawing_color_purple, drawing_color_red)
        colorButtons.forEach { button ->
            button.setStrokeColorResource(R.color.blue)
        }
    }

    private fun changeToolsVisibility(newVisibility: Int) {
        handwriting_editor_tools_menu.visibility = newVisibility
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

    @SuppressLint("ClickableViewAccessibility")
    private fun setUpToolbarListeners() {
        drawing_color_white.setOnClickListener {
            handwritingEditorViewModel.handleNewColorSelected(android.R.color.white)
        }
        drawing_color_blue.setOnClickListener {
            handwritingEditorViewModel.handleNewColorSelected(android.R.color.holo_blue_light)
        }

        drawing_color_green.setOnClickListener {
            handwritingEditorViewModel.handleNewColorSelected(android.R.color.holo_green_light)
        }

        drawing_color_orange.setOnClickListener {
            handwritingEditorViewModel.handleNewColorSelected(android.R.color.holo_orange_light)
        }

        drawing_color_purple.setOnClickListener {
            handwritingEditorViewModel.handleNewColorSelected(android.R.color.holo_purple)
        }

        drawing_color_red.setOnClickListener {
            handwritingEditorViewModel.handleNewColorSelected(android.R.color.holo_red_light)
        }

        drawing_erase.setOnClickListener {
            handwritingEditorViewModel.handleNewToolSelected(HandwritingEditorView.Tool.ERASER)
        }

        drawing_pen.setOnClickListener {
            handwritingEditorViewModel.handleNewToolSelected(HandwritingEditorView.Tool.PEN)
        }

        handwriting_editor_pen_small.setOnClickListener {
            handwritingEditorViewModel.handleNewPenSizeSelected(HandwritingEditorView.PenSize.SMALL)
        }

        handwriting_editor_pen_medium.setOnClickListener {
            handwritingEditorViewModel.handleNewPenSizeSelected(HandwritingEditorView.PenSize.MEDIUM)
        }

        handwriting_editor_pen_large.setOnClickListener {
            handwritingEditorViewModel.handleNewPenSizeSelected(HandwritingEditorView.PenSize.LARGE)
        }

        gestureDetector = GestureDetector(
            applicationContext,
            object : GestureDetector.OnGestureListener {
                override fun onDown(e: MotionEvent?): Boolean = true

                override fun onShowPress(e: MotionEvent?) {}

                override fun onSingleTapUp(e: MotionEvent?): Boolean {
                    handwritingEditorViewModel.handleTouchEvent(handwriting_editor_tools_menu.visibility)
                    return true
                }

                override fun onScroll(
                    e1: MotionEvent?,
                    e2: MotionEvent?,
                    distanceX: Float,
                    distanceY: Float
                ): Boolean = true

                override fun onLongPress(e: MotionEvent?) {}

                override fun onFling(
                    e1: MotionEvent?,
                    e2: MotionEvent?,
                    velocityX: Float,
                    velocityY: Float
                ): Boolean = true
            }
        )

        handwriting_editor.setOnTouchListener { _, event ->
            if (event != null && event.getToolType(0) == MotionEvent.TOOL_TYPE_FINGER) {
                gestureDetector.onTouchEvent(event)
            }

            false
        }
    }

    private fun startBackgroundSave() { // could this crash if the threads take too long to finish and they keep saving?!
        autoSaveTimer.schedule(
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
        autoSaveTimer.cancel()
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
