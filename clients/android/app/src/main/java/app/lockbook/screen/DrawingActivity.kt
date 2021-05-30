package app.lockbook.screen

import android.animation.Animator
import android.annotation.SuppressLint
import android.content.res.ColorStateList
import android.content.res.Configuration
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.GestureDetector
import android.view.MotionEvent
import android.view.View
import android.widget.ImageButton
import android.widget.SeekBar
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.widget.AppCompatSeekBar
import androidx.lifecycle.ViewModelProvider
import app.lockbook.R
import app.lockbook.databinding.ActivityDrawingBinding
import app.lockbook.model.AlertModel
import app.lockbook.model.DrawingViewModel
import app.lockbook.model.OnFinishAlert
import app.lockbook.modelfactory.DrawingViewModelFactory
import app.lockbook.screen.TextEditorActivity.Companion.TEXT_EDITOR_BACKGROUND_SAVE_PERIOD
import app.lockbook.ui.DrawingView
import app.lockbook.util.*
import com.google.android.material.button.MaterialButton
import java.util.*

class DrawingActivity : AppCompatActivity() {

    private var _binding: ActivityDrawingBinding? = null
    // This property is only valid between onCreateView and
    // onDestroyView.
    private val binding get() = _binding!!
    private val whiteButton get() = binding.drawingToolbar.findViewById<MaterialButton>(R.id.drawing_color_white)
    private val blackButton get() = binding.drawingToolbar.findViewById<MaterialButton>(R.id.drawing_color_black)
    private val blueButton get() = binding.drawingToolbar.findViewById<MaterialButton>(R.id.drawing_color_blue)
    private val cyanButton get() = binding.drawingToolbar.findViewById<MaterialButton>(R.id.drawing_color_cyan)
    private val greenButton get() = binding.drawingToolbar.findViewById<MaterialButton>(R.id.drawing_color_green)
    private val magentaButton get() = binding.drawingToolbar.findViewById<MaterialButton>(R.id.drawing_color_magenta)
    private val yellowButton get() = binding.drawingToolbar.findViewById<MaterialButton>(R.id.drawing_color_yellow)
    private val redButton get() = binding.drawingToolbar.findViewById<MaterialButton>(R.id.drawing_color_red)

    private val eraser get() = binding.drawingToolbar.findViewById<ImageButton>(R.id.drawing_erase)
    private val penSizeChooser get() = binding.drawingToolbar.findViewById<AppCompatSeekBar>(R.id.drawing_pen_size)
    private val penSizeIndicator get() = binding.drawingToolbar.findViewById<TextView>(R.id.drawing_pen_size_marker)

    private val drawingView get() = binding.drawingView
    private lateinit var drawingViewModel: DrawingViewModel
    private var isFirstLaunch = true

    private var autoSaveTimer = Timer()
    private val handler = Handler(requireNotNull(Looper.myLooper()))
    private lateinit var id: String
    private lateinit var gestureDetector: GestureDetector

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        _binding = ActivityDrawingBinding.inflate(layoutInflater)
        setContentView(binding.root)

        val maybeId = intent.getStringExtra("id")

        if (maybeId == null) {
            AlertModel.errorHasOccurred(binding.drawingLayout, "Unable to get get file id.", OnFinishAlert.DoSomethingOnFinishAlert(::finish))
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
            AlertModel.errorHasOccurred(binding.drawingLayout, errorText, OnFinishAlert.DoSomethingOnFinishAlert(::finish))
        }

        drawingViewModel.unexpectedErrorHasOccurred.observe(
            this
        ) { errorText ->
            AlertModel.unexpectedCoreErrorHasOccurred(this, errorText, OnFinishAlert.DoSomethingOnFinishAlert(::finish))
        }

        drawingViewModel.drawableReady.observe(
            this
        ) {
            initializeDrawing()
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

    override fun onPause() {
        super.onPause()
        drawingView.stopThread()
    }

    override fun onResume() {
        super.onResume()
        drawingView.startThread()
    }

    override fun onDestroy() {
        super.onDestroy()
        autoSaveTimer.cancel()
        autoSaveTimer.purge()
        if (!isFirstLaunch) {
            drawingViewModel.backupDrawing = drawingView.drawing
            drawingViewModel.saveDrawing(drawingView.drawing.clone())
        }
    }

    private fun selectNewTool(oldTool: DrawingView.Tool?, newTool: DrawingView.Tool) {
        when (oldTool) {
            is DrawingView.Pen -> {
                val previousButton = when (oldTool.colorAlias) {
                    ColorAlias.White -> whiteButton
                    ColorAlias.Blue -> blueButton
                    ColorAlias.Green -> greenButton
                    ColorAlias.Yellow -> yellowButton
                    ColorAlias.Magenta -> magentaButton
                    ColorAlias.Red -> redButton
                    ColorAlias.Black -> blackButton
                    ColorAlias.Cyan -> cyanButton
                }.exhaustive

                previousButton.strokeWidth = 0
            }
            is DrawingView.Eraser -> {
                eraser.setImageResource(R.drawable.ic_eraser_outline)
                drawingView.isErasing = false
            }
            null -> {}
            else -> AlertModel.errorHasOccurred(binding.drawingLayout, "Unable to recognize previous tool.", OnFinishAlert.DoNothingOnFinishAlert)
        }

        when (newTool) {
            is DrawingView.Pen -> {
                val newButton = when (newTool.colorAlias) {
                    ColorAlias.White -> whiteButton
                    ColorAlias.Blue -> blueButton
                    ColorAlias.Green -> greenButton
                    ColorAlias.Yellow -> yellowButton
                    ColorAlias.Magenta -> magentaButton
                    ColorAlias.Red -> redButton
                    ColorAlias.Black -> blackButton
                    ColorAlias.Cyan -> cyanButton
                }.exhaustive

                newButton.strokeWidth = 4
                drawingView.strokeColor = newTool.colorAlias
            }
            is DrawingView.Eraser -> {
                eraser.setImageResource(R.drawable.ic_eraser_filled)
                drawingView.isErasing = true
            }
            else -> AlertModel.errorHasOccurred(binding.drawingLayout, "Unable to recognize new tool.", OnFinishAlert.DoNothingOnFinishAlert)
        }.exhaustive
    }

    private fun selectedNewPenSize(
        newPenSize: Int
    ) {
        val penSizeSeekBar = penSizeChooser
        if (penSizeSeekBar.progress + 1 != newPenSize) {
            penSizeSeekBar.progress = newPenSize
        }

        drawingView.setPenSize(newPenSize)
    }

    private fun setUpToolbarDefaults() {
        val colorButtons = listOf(whiteButton, blackButton, blueButton, greenButton, yellowButton, magentaButton, redButton, cyanButton)
        colorButtons.forEach { button ->
            button.setStrokeColorResource(R.color.blue)
        }
    }

    private fun changeToolsVisibility(newVisibility: Int) {
        val onAnimationEnd = object : Animator.AnimatorListener {
            override fun onAnimationStart(animation: Animator?) {
                if (newVisibility == View.VISIBLE) {
                    binding.drawingToolbar.visibility = newVisibility
                }
            }
            override fun onAnimationEnd(animation: Animator?) {
                if (newVisibility == View.GONE) {
                    binding.drawingToolbar.visibility = newVisibility
                }
            }

            override fun onAnimationCancel(animation: Animator?) {}
            override fun onAnimationRepeat(animation: Animator?) {}
        }

        binding.drawingToolbar.animate().setDuration(300).alpha(if (newVisibility == View.VISIBLE) 1f else 0f).setListener(onAnimationEnd).start()
    }

    private fun initializeDrawing() {
        binding.drawingProgressBar.visibility = View.GONE

        val drawing = drawingViewModel.backupDrawing

        if (drawing == null) {
            AlertModel.errorHasOccurred(binding.drawingLayout, "Unable to get backup drawing.", OnFinishAlert.DoNothingOnFinishAlert)
            return
        }

        val colorAliasInARGB = EnumMap(drawing.themeToARGBColors(resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK))

        val white = colorAliasInARGB[ColorAlias.White]
        val black = colorAliasInARGB[ColorAlias.Black]
        val red = colorAliasInARGB[ColorAlias.Red]
        val green = colorAliasInARGB[ColorAlias.Green]
        val cyan = colorAliasInARGB[ColorAlias.Cyan]
        val magenta = colorAliasInARGB[ColorAlias.Magenta]
        val blue = colorAliasInARGB[ColorAlias.Blue]
        val yellow = colorAliasInARGB[ColorAlias.Yellow]

        if (white == null || black == null || red == null || green == null || cyan == null || magenta == null || blue == null || yellow == null) {
            AlertModel.errorHasOccurred(binding.drawingLayout, "Unable to get 1 or more colors from theme.", OnFinishAlert.DoNothingOnFinishAlert)
            return
        }

        drawingView.colorAliasInARGB = colorAliasInARGB

        whiteButton.backgroundTintList = ColorStateList(arrayOf(intArrayOf()), intArrayOf(white))
        blackButton.backgroundTintList = ColorStateList(arrayOf(intArrayOf()), intArrayOf(black))
        redButton.backgroundTintList = ColorStateList(arrayOf(intArrayOf()), intArrayOf(red))
        greenButton.backgroundTintList = ColorStateList(arrayOf(intArrayOf()), intArrayOf(green))
        cyanButton.backgroundTintList = ColorStateList(arrayOf(intArrayOf()), intArrayOf(cyan))
        magentaButton.backgroundTintList = ColorStateList(arrayOf(intArrayOf()), intArrayOf(magenta))
        blueButton.backgroundTintList = ColorStateList(arrayOf(intArrayOf()), intArrayOf(blue))
        yellowButton.backgroundTintList = ColorStateList(arrayOf(intArrayOf()), intArrayOf(yellow))

        binding.drawingToolbar.visibility = View.VISIBLE

        isFirstLaunch = false
        binding.drawingLoadingView.visibility = View.GONE
        drawingView.initializeWithDrawing(drawing)
    }

    private fun startDrawing() {
        binding.drawingProgressBar.visibility = View.VISIBLE

        if (drawingViewModel.backupDrawing == null) {
            drawingViewModel.getDrawing(id)
        } else {
            initializeDrawing()
        }
    }

    @SuppressLint("ClickableViewAccessibility")
    private fun setUpToolbarListeners() {
        whiteButton.setOnClickListener {
            drawingViewModel.handleNewToolSelected(DrawingView.Pen(ColorAlias.White))
        }

        blackButton.setOnClickListener {
            drawingViewModel.handleNewToolSelected(DrawingView.Pen(ColorAlias.Black))
        }

        blueButton.setOnClickListener {
            drawingViewModel.handleNewToolSelected(DrawingView.Pen(ColorAlias.Blue))
        }

        greenButton.setOnClickListener {
            drawingViewModel.handleNewToolSelected(DrawingView.Pen(ColorAlias.Green))
        }

        yellowButton.setOnClickListener {
            drawingViewModel.handleNewToolSelected(DrawingView.Pen(ColorAlias.Yellow))
        }

        magentaButton.setOnClickListener {
            drawingViewModel.handleNewToolSelected(DrawingView.Pen(ColorAlias.Magenta))
        }

        redButton.setOnClickListener {
            drawingViewModel.handleNewToolSelected(DrawingView.Pen(ColorAlias.Red))
        }

        cyanButton.setOnClickListener {
            drawingViewModel.handleNewToolSelected(DrawingView.Pen(ColorAlias.Cyan))
        }

        eraser.setOnClickListener {
            drawingViewModel.handleNewToolSelected(DrawingView.Eraser)
        }

        penSizeChooser.setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
            override fun onProgressChanged(seekBar: SeekBar?, progress: Int, fromUser: Boolean) {
                val adjustedProgress = progress + 1
                penSizeIndicator.text = adjustedProgress.toString()
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
                    drawingViewModel.handleTouchEvent(binding.drawingToolbar.visibility)
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

        drawingView.setOnTouchListener { _, event ->
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
                                drawingView.drawing.clone()
                            )
                        }
                    }
                }
            },
            1000,
            TEXT_EDITOR_BACKGROUND_SAVE_PERIOD
        )
    }
}
