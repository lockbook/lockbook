package app.lockbook.screen

import android.animation.Animator
import android.annotation.SuppressLint
import android.content.res.ColorStateList
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.GestureDetector
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.View
import android.widget.SeekBar
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.widget.AppCompatSeekBar
import androidx.lifecycle.ViewModelProvider
import app.lockbook.App.Companion.UNEXPECTED_ERROR
import app.lockbook.R
import app.lockbook.model.DrawingViewModel
import app.lockbook.modelfactory.DrawingViewModelFactory
import app.lockbook.screen.TextEditorActivity.Companion.TEXT_EDITOR_BACKGROUND_SAVE_PERIOD
import app.lockbook.ui.DrawingView
import app.lockbook.util.*
import com.google.android.material.snackbar.Snackbar
import kotlinx.android.synthetic.main.activity_drawing.*
import kotlinx.android.synthetic.main.toolbar_drawing.*
import java.util.*

class DrawingActivity : AppCompatActivity() {
    private lateinit var drawingViewModel: DrawingViewModel
    private var isFirstLaunch = true
    private val surfaceViewReadyCallback = object : SurfaceHolder.Callback {
        override fun surfaceCreated(holder: SurfaceHolder) {
            if (!isFirstLaunch) {
                drawing_view.startThread()
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
                DrawingViewModelFactory(application, id)
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
            drawing_view.holder.addCallback(surfaceViewReadyCallback)

            if (!drawing_view.holder.isCreating) {
                addDrawingToView()
            }
        }

        drawingViewModel.setToolsVisibility.observe(
            this
        ) { newVisibility ->
            changeToolsVisibility(newVisibility)
        }

        drawingViewModel.selectNewTool.observe(
            this
        ) { tools ->
            selectNewTool(tools.first, tools.second)
        }

        drawingViewModel.selectedNewPenSize.observe(
            this
        ) { penSize ->
            selectedNewPenSize(penSize)
        }

        startDrawing()
        startBackgroundSave()
        setUpToolbarListeners()
        setUpToolbarDefaults()
    }

    override fun onRestart() {
        super.onRestart()
        drawing_view.restartThread()
    }

    override fun onPause() {
        super.onPause()
        drawing_view.endThread()
    }

    override fun onDestroy() {
        super.onDestroy()
        drawing_view.endThread()
        autoSaveTimer.cancel()
        if (!isFirstLaunch) {
            drawingViewModel.backupDrawing = drawing_view.drawing
            drawingViewModel.saveDrawing(drawing_view.drawing)
        }
    }

    private fun selectNewTool(oldTool: DrawingView.Tool?, newTool: DrawingView.Tool) {
        when (oldTool) {
            is DrawingView.Pen -> {
                val previousButton = when (oldTool.colorAlias) {
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
            is DrawingView.Eraser -> {
                drawing_erase.setImageResource(R.drawable.ic_eraser_outline)
            }
            null -> {}
            else -> unexpectedErrorHasOccurred("A tool previously used is unrecognized.")
        }

        when (newTool) {
            is DrawingView.Pen -> {
                val newButton = when (newTool.colorAlias) {
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
                drawing_view.strokeColor = newTool.colorAlias
            }
            is DrawingView.Eraser -> {
                drawing_erase.setImageResource(R.drawable.ic_eraser_filled)
            }
            else -> unexpectedErrorHasOccurred("Tried to use unknown tool.")
        }.exhaustive
    }

    private fun selectedNewPenSize(
        newPenSize: Int
    ) {
        val penSizeSeekBar = drawing_pen_size as AppCompatSeekBar
        if (penSizeSeekBar.progress + 1 != newPenSize) {
            penSizeSeekBar.progress = newPenSize
        }

        drawing_view.setPenSize(newPenSize)
    }

    private fun setUpToolbarDefaults() {
        val colorButtons = listOf(drawing_color_white, drawing_color_black, drawing_color_blue, drawing_color_green, drawing_color_yellow, drawing_color_magenta, drawing_color_red, drawing_color_cyan)
        colorButtons.forEach { button ->
            button.setStrokeColorResource(R.color.blue)
        }
    }

    private fun changeToolsVisibility(newVisibility: Int) {
        val onAnimationEnd = object : Animator.AnimatorListener {
            override fun onAnimationStart(animation: Animator?) {
                if (newVisibility == View.VISIBLE && drawing_view.isTouchable) {
                    drawing_tools_menu.visibility = newVisibility
                }
            }
            override fun onAnimationEnd(animation: Animator?) {
                if (newVisibility == View.GONE && drawing_view.isTouchable) {
                    drawing_tools_menu.visibility = newVisibility
                }
            }

            override fun onAnimationCancel(animation: Animator?) {}
            override fun onAnimationRepeat(animation: Animator?) {}
        }

        drawing_tools_menu.animate().setDuration(300).alpha(if (newVisibility == View.VISIBLE) 1f else 0f).setListener(onAnimationEnd).start()
    }

    private fun addDrawingToView() {
        drawing_view.isTouchable = true
        drawing_progress_bar.visibility = View.GONE

        val drawing = drawingViewModel.backupDrawing

        if (drawing == null) {
            unexpectedErrorHasOccurred("Unable to get color from theme.")
            return
        }

        drawing_view.theme = drawing.theme ?: DEFAULT_THEME
        drawing_view.colorAliasInARGB = EnumMap(Drawing.themeToARGBColors(drawing_view.theme))

        val white = drawing_view.colorAliasInARGB[ColorAlias.White]
        val black = drawing_view.colorAliasInARGB[ColorAlias.Black]
        val red = drawing_view.colorAliasInARGB[ColorAlias.Red]
        val green = drawing_view.colorAliasInARGB[ColorAlias.Green]
        val cyan = drawing_view.colorAliasInARGB[ColorAlias.Cyan]
        val magenta = drawing_view.colorAliasInARGB[ColorAlias.Magenta]
        val blue = drawing_view.colorAliasInARGB[ColorAlias.Blue]
        val yellow = drawing_view.colorAliasInARGB[ColorAlias.Yellow]

        if (white == null || black == null || red == null || green == null || cyan == null || magenta == null || blue == null || yellow == null) {
            unexpectedErrorHasOccurred("Unable to get 1 or more colors from theme.")
            return
        }

        drawing_color_white.backgroundTintList = ColorStateList(arrayOf(intArrayOf()), intArrayOf(white))
        drawing_color_black.backgroundTintList = ColorStateList(arrayOf(intArrayOf()), intArrayOf(black))
        drawing_color_red.backgroundTintList = ColorStateList(arrayOf(intArrayOf()), intArrayOf(red))
        drawing_color_green.backgroundTintList = ColorStateList(arrayOf(intArrayOf()), intArrayOf(green))
        drawing_color_cyan.backgroundTintList = ColorStateList(arrayOf(intArrayOf()), intArrayOf(cyan))
        drawing_color_magenta.backgroundTintList = ColorStateList(arrayOf(intArrayOf()), intArrayOf(magenta))
        drawing_color_blue.backgroundTintList = ColorStateList(arrayOf(intArrayOf()), intArrayOf(blue))
        drawing_color_yellow.backgroundTintList = ColorStateList(arrayOf(intArrayOf()), intArrayOf(yellow))

        drawing_tools_menu.visibility = View.VISIBLE

        isFirstLaunch = false
        drawing_view.initializeWithDrawing(drawing)
    }

    private fun startDrawing() {
        drawing_progress_bar.visibility = View.VISIBLE

        if (drawingViewModel.backupDrawing == null) {
            drawingViewModel.getDrawing(id)
        } else {
            drawing_view.holder.addCallback(surfaceViewReadyCallback)
        }
    }

    @SuppressLint("ClickableViewAccessibility")
    private fun setUpToolbarListeners() {
        drawing_color_white.setOnClickListener {
            drawingViewModel.handleNewToolSelected(DrawingView.Pen(ColorAlias.White))
        }

        drawing_color_black.setOnClickListener {
            drawingViewModel.handleNewToolSelected(DrawingView.Pen(ColorAlias.Black))
        }

        drawing_color_blue.setOnClickListener {
            drawingViewModel.handleNewToolSelected(DrawingView.Pen(ColorAlias.Blue))
        }

        drawing_color_green.setOnClickListener {
            drawingViewModel.handleNewToolSelected(DrawingView.Pen(ColorAlias.Green))
        }

        drawing_color_yellow.setOnClickListener {
            drawingViewModel.handleNewToolSelected(DrawingView.Pen(ColorAlias.Yellow))
        }

        drawing_color_magenta.setOnClickListener {
            drawingViewModel.handleNewToolSelected(DrawingView.Pen(ColorAlias.Magenta))
        }

        drawing_color_red.setOnClickListener {
            drawingViewModel.handleNewToolSelected(DrawingView.Pen(ColorAlias.Red))
        }

        drawing_color_cyan.setOnClickListener {
            drawingViewModel.handleNewToolSelected(DrawingView.Pen(ColorAlias.Cyan))
        }

        drawing_erase.setOnClickListener {
            drawingViewModel.handleNewToolSelected(DrawingView.Eraser)
        }

        (drawing_pen_size as AppCompatSeekBar).setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
            override fun onProgressChanged(seekBar: SeekBar?, progress: Int, fromUser: Boolean) {
                val adjustedProgress = progress + 1
                drawing_pen_size_marker.text = adjustedProgress.toString()
                drawingViewModel.handleNewPenSizeSelected(adjustedProgress)
            }

            override fun onStartTrackingTouch(seekBar: SeekBar?) {}

            override fun onStopTrackingTouch(seekBar: SeekBar?) {}
        })

        gestureDetector = GestureDetector(
            applicationContext,
            object : GestureDetector.OnGestureListener {
                override fun onDown(e: MotionEvent?): Boolean = true

                override fun onShowPress(e: MotionEvent?) {}

                override fun onSingleTapUp(e: MotionEvent?): Boolean {
                    drawingViewModel.handleTouchEvent(drawing_tools_menu.visibility)
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

        drawing_view.setOnTouchListener { _, event ->
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
                                drawing_view.drawing.clone()
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
        Snackbar.make(drawing_layout, error, Snackbar.LENGTH_SHORT)
            .addCallback(object : Snackbar.Callback() {
                override fun onDismissed(transientBottomBar: Snackbar?, event: Int) {
                    super.onDismissed(transientBottomBar, event)
                    finish()
                }
            }).show()
    }

    fun unexpectedErrorHasOccurred(error: String) {
        AlertDialog.Builder(this, R.style.Main_Widget_Dialog)
            .setTitle(UNEXPECTED_ERROR)
            .setMessage(error)
            .setOnCancelListener {
                finish()
            }
            .show()
    }
}
