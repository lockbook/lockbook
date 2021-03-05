package app.lockbook.screen

import android.animation.Animator
import android.annotation.SuppressLint
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.GestureDetector
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.View
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.ViewModelProvider
import app.lockbook.R
import app.lockbook.model.DrawingViewModel
import app.lockbook.modelfactory.HandwritingEditorViewModelFactory
import app.lockbook.ui.DrawingView
import app.lockbook.util.*
import app.lockbook.util.Messages.UNEXPECTED_ERROR
import com.google.android.material.snackbar.Snackbar
import kotlinx.android.synthetic.main.activity_drawing.*
import java.util.*

class DrawingActivity : AppCompatActivity() {
    private lateinit var drawingViewModel: DrawingViewModel
    private var isFirstLaunch = true
    private val surfaceViewReadyCallback = object : SurfaceHolder.Callback {
        override fun surfaceCreated(holder: SurfaceHolder) {
            if (!isFirstLaunch) {
                handwriting_editor.startThread()
            } else {
                addDrawingToView()
            }
        }

        override fun surfaceChanged(
            holder: SurfaceHolder,
            format: Int,
            width: Int,
            height: Int
        ) {
        }

        override fun surfaceDestroyed(holder: SurfaceHolder) {
        }
    }

    private var autoSaveTimer = Timer()
    private val handler = Handler(requireNotNull(Looper.myLooper()))
    private lateinit var id: String
    private lateinit var gestureDetector: GestureDetector

    override fun onCreate(savedInstanceState: Bundle?) {

        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_drawing)

        val maybeId = intent.getStringExtra("id")

        if (maybeId == null) {
            errorHasOccurred("Unable to retrieve id.")
            finish()
            return
        }

        id = maybeId

        drawingViewModel =
            ViewModelProvider(
                this,
                HandwritingEditorViewModelFactory(application, id)
            ).get(DrawingViewModel::class.java)

        drawingViewModel.errorHasOccurred.observe(
            this
        ) { errorText ->
            errorHasOccurred(errorText)
        }

        drawingViewModel.unexpectedErrorHasOccurred.observe(
            this
        ) { errorText ->
            unexpectedErrorHasOccurred(errorText)
        }

        drawingViewModel.drawableReady.observe(
            this
        ) {
            handwriting_editor.holder.addCallback(surfaceViewReadyCallback)

            if (!handwriting_editor.holder.isCreating) {
                addDrawingToView()
            }
        }

        drawingViewModel.selectNewColor.observe(
            this
        ) { colors ->
            selectNewColor(colors.first, colors.second)
        }

        drawingViewModel.setToolsVisibility.observe(
            this
        ) { newVisibility ->
            changeToolsVisibility(newVisibility)
        }

        drawingViewModel.selectNewTool.observe(
            this
        ) { tools ->
            selectNewTool(tools.second)
        }

        drawingViewModel.selectedNewPenSize.observe(
            this
        ) { penSizes ->
            selectedNewPenSize(penSizes.first, penSizes.second)
        }

        startDrawing()
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

    override fun onDestroy() {
        super.onDestroy()
        handwriting_editor.endThread()
        autoSaveTimer.cancel()
        if (!isFirstLaunch) {
            drawingViewModel.backupDrawing = handwriting_editor.drawingModel
            drawingViewModel.saveDrawing(handwriting_editor.drawingModel)
        }
    }

    private fun selectNewColor(oldColor: ColorAlias?, newColor: ColorAlias) {
        if (oldColor != null) {
            val previousButton = when (oldColor) {
                ColorAlias.White -> drawing_color_white
                ColorAlias.Blue -> drawing_color_blue
                ColorAlias.Green -> drawing_color_green
                ColorAlias.Yellow -> drawing_color_yellow
                ColorAlias.Magenta -> drawing_color_magenta
                ColorAlias.Red -> drawing_color_red
                ColorAlias.Black -> drawing_color_black
                ColorAlias.Cyan -> drawing_color_cyan
            }.exhaustive

            previousButton.strokeWidth = 0
        }

        val newButton = when (newColor) {
            ColorAlias.White -> drawing_color_white
            ColorAlias.Blue -> drawing_color_blue
            ColorAlias.Green -> drawing_color_green
            ColorAlias.Yellow -> drawing_color_yellow
            ColorAlias.Magenta -> drawing_color_magenta
            ColorAlias.Red -> drawing_color_red
            ColorAlias.Black -> drawing_color_black
            ColorAlias.Cyan -> drawing_color_cyan
        }.exhaustive

        newButton.strokeWidth = 4
        handwriting_editor.currentColor = newColor
    }

    private fun selectNewTool(newTool: DrawingView.Tool) {
        if (newTool == DrawingView.Tool.PEN) {
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
            oldPenSize: DrawingView.PenSize?,
            newPenSize: DrawingView.PenSize
    ) {
        if (oldPenSize != null) {
            val previousButton = when (oldPenSize) {
                DrawingView.PenSize.SMALL -> handwriting_editor_pen_small
                DrawingView.PenSize.MEDIUM -> handwriting_editor_pen_medium
                DrawingView.PenSize.LARGE -> handwriting_editor_pen_large
            }.exhaustive

            previousButton.setBackgroundResource(0)
        }

        val newButton = when (newPenSize) {
            DrawingView.PenSize.SMALL -> handwriting_editor_pen_small
            DrawingView.PenSize.MEDIUM -> handwriting_editor_pen_medium
            DrawingView.PenSize.LARGE -> handwriting_editor_pen_large
        }.exhaustive

        newButton.setBackgroundResource(R.drawable.item_border)
        handwriting_editor.setPenSize(newPenSize)
    }

    private fun setUpToolbarDefaults() {
        val colorButtons = listOf(drawing_color_white, drawing_color_blue, drawing_color_green, drawing_color_yellow, drawing_color_magenta, drawing_color_red)
        colorButtons.forEach { button ->
            button.setStrokeColorResource(R.color.blue)
        }
    }

    private fun changeToolsVisibility(newVisibility: Int) {
        val onAnimationEnd = object : Animator.AnimatorListener {
            override fun onAnimationStart(animation: Animator?) {
                if (newVisibility == View.VISIBLE) {
                    handwriting_editor_tools_menu.visibility = newVisibility
                }
            }
            override fun onAnimationEnd(animation: Animator?) {
                if (newVisibility == View.GONE) {
                    handwriting_editor_tools_menu.visibility = newVisibility
                }
            }

            override fun onAnimationCancel(animation: Animator?) {}
            override fun onAnimationRepeat(animation: Animator?) {}
        }

        handwriting_editor_tools_menu.animate().setDuration(300).alpha(if (newVisibility == View.VISIBLE) 1f else 0f).setListener(onAnimationEnd).start()
    }

    private fun addDrawingToView() {
        handwriting_editor.isTouchable = true
        handwriting_editor_progress_bar.visibility = View.GONE
        isFirstLaunch = false
        handwriting_editor.initializeWithDrawing(drawingViewModel.backupDrawing)
    }

    private fun startDrawing() {
        handwriting_editor_progress_bar.visibility = View.VISIBLE

        if (drawingViewModel.backupDrawing == null) {
            drawingViewModel.getDrawing(id)
        } else {
            handwriting_editor.holder.addCallback(surfaceViewReadyCallback)
        }
    }

    @SuppressLint("ClickableViewAccessibility")
    private fun setUpToolbarListeners() {
        drawing_color_white.setOnClickListener {
            drawingViewModel.handleNewColorSelected(ColorAlias.White)
        }
        drawing_color_blue.setOnClickListener {
            drawingViewModel.handleNewColorSelected(ColorAlias.Blue)
        }

        drawing_color_green.setOnClickListener {
            drawingViewModel.handleNewColorSelected(ColorAlias.Green)
        }

        drawing_color_yellow.setOnClickListener {
            drawingViewModel.handleNewColorSelected(ColorAlias.Yellow)
        }

        drawing_color_magenta.setOnClickListener {
            drawingViewModel.handleNewColorSelected(ColorAlias.Magenta)
        }

        drawing_color_red.setOnClickListener {
            drawingViewModel.handleNewColorSelected(ColorAlias.Red)
        }

        drawing_erase.setOnClickListener {
            drawingViewModel.handleNewToolSelected(DrawingView.Tool.ERASER)
        }

        drawing_pen.setOnClickListener {
            drawingViewModel.handleNewToolSelected(DrawingView.Tool.PEN)
        }

        handwriting_editor_pen_small.setOnClickListener {
            drawingViewModel.handleNewPenSizeSelected(DrawingView.PenSize.SMALL)
        }

        handwriting_editor_pen_medium.setOnClickListener {
            drawingViewModel.handleNewPenSizeSelected(DrawingView.PenSize.MEDIUM)
        }

        handwriting_editor_pen_large.setOnClickListener {
            drawingViewModel.handleNewPenSizeSelected(DrawingView.PenSize.LARGE)
        }

        gestureDetector = GestureDetector(
            applicationContext,
            object : GestureDetector.OnGestureListener {
                override fun onDown(e: MotionEvent?): Boolean = true

                override fun onShowPress(e: MotionEvent?) {}

                override fun onSingleTapUp(e: MotionEvent?): Boolean {
                    drawingViewModel.handleTouchEvent(handwriting_editor_tools_menu.visibility)
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
                        if (!isFirstLaunch) {
                            drawingViewModel.saveDrawing(
                                Drawing(
                                    handwriting_editor.drawingModel.scale,
                                    handwriting_editor.drawingModel.translationX,
                                    handwriting_editor.drawingModel.translationY,
                                    handwriting_editor.drawingModel.strokes.map { stroke ->
                                        Stroke(
                                            stroke.pointsX.toMutableList(),
                                            stroke.pointsY.toMutableList(),
                                            stroke.pointsGirth.toMutableList(),
                                            stroke.color,
                                            stroke.alpha
                                        )
                                    }.toMutableList(),
                                    handwriting_editor.drawingModel.theme
                                )
                            )
                        }
                    }
                }
            },
            1000,
            TEXT_EDITOR_BACKGROUND_SAVE_PERIOD
        )
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
        AlertDialog.Builder(this, R.style.Main_Widget_Dialog)
            .setTitle(UNEXPECTED_ERROR)
            .setMessage(error)
            .setOnCancelListener {
                finish()
            }
            .show()
    }
}
