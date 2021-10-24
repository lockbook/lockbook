package app.lockbook.screen

import android.animation.Animator
import android.annotation.SuppressLint
import android.content.res.ColorStateList
import android.content.res.Configuration
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.*
import android.widget.SeekBar
import androidx.appcompat.widget.AppCompatSeekBar
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import app.lockbook.R
import app.lockbook.databinding.FragmentDrawingBinding
import app.lockbook.model.*
import app.lockbook.ui.DrawingView
import app.lockbook.util.ColorAlias
import app.lockbook.util.exhaustive
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import java.lang.ref.WeakReference
import java.util.*

class DrawingFragment : Fragment() {

    private var _binding: FragmentDrawingBinding? = null
    val binding get() = _binding!!

    private val whiteButton get() = binding.drawingToolbar.drawingColorWhite
    private val blackButton get() = binding.drawingToolbar.drawingColorBlack
    private val blueButton get() = binding.drawingToolbar.drawingColorBlue
    private val cyanButton get() = binding.drawingToolbar.drawingColorCyan
    private val greenButton get() = binding.drawingToolbar.drawingColorGreen
    private val magentaButton get() = binding.drawingToolbar.drawingColorMagenta
    private val yellowButton get() = binding.drawingToolbar.drawingColorYellow
    private val redButton get() = binding.drawingToolbar.drawingColorRed

    private val eraser get() = binding.drawingToolbar.drawingErase
    private val penSizeChooser get() = binding.drawingToolbar.drawingPenSize as AppCompatSeekBar
    private val penSizeIndicator get() = binding.drawingToolbar.drawingPenSizeMarker

    private val toolbar get() = binding.drawingToolbar.drawingToolsMenu

    private val activityModel: StateViewModel by activityViewModels()
    private val drawingView get() = binding.drawingView
    private val model: DrawingViewModel by viewModels(
        factoryProducer = {
            object : ViewModelProvider.Factory {
                override fun <T : ViewModel?> create(modelClass: Class<T>): T {
                    if (modelClass.isAssignableFrom(DrawingViewModel::class.java))
                        return DrawingViewModel(
                            requireActivity().application,
                            activityModel.detailsScreen!!.fileMetadata.id,
                            (activityModel.detailsScreen as DetailsScreen.Drawing).drawing
                        ) as T
                    throw IllegalArgumentException("Unknown ViewModel class")
                }
            }
        }
    )

    private var isFirstLaunch = true

    private var autoSaveTimer = Timer()
    private val handler = Handler(requireNotNull(Looper.myLooper()))
    private lateinit var gestureDetector: GestureDetector

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = FragmentDrawingBinding.inflate(inflater, container, false)

        model.drawingReady.observe(
            viewLifecycleOwner
        ) {
            initializeDrawing()
        }

        initializeDrawing()
        startBackgroundSave()
        setUpToolbarListeners()
        setUpToolbarDefaults()

        return binding.root
    }

    override fun onPause() {
        super.onPause()
        drawingView.stopThread()
    }

    override fun onResume() {
        super.onResume()
        if (drawingView.isThreadAvailable && drawingView.isDrawingAvailable) {
            drawingView.startThread()
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        autoSaveTimer.cancel()
        autoSaveTimer.purge()
        if (!isFirstLaunch) {
            model.persistentDrawing = drawingView.drawing
            model.saveDrawing(drawingView.drawing.clone())
        }
    }

    private fun selectNewTool(newTool: DrawingView.Tool) {
        when (val oldTool = model.selectedTool) {
            is DrawingView.Tool.Pen -> {
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
            is DrawingView.Tool.Eraser -> {
                eraser.setImageResource(R.drawable.ic_eraser_outline)
                drawingView.strokeState.isErasing = false
            }
        }

        when (newTool) {
            is DrawingView.Tool.Pen -> {
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
                drawingView.strokeState.strokeColor = newTool.colorAlias
            }
            is DrawingView.Tool.Eraser -> {
                eraser.setImageResource(R.drawable.ic_eraser_filled)
                drawingView.strokeState.isErasing = true
            }
        }.exhaustive

        model.selectedTool = newTool
    }

    private fun setUpToolbarDefaults() {
        val colorButtons = listOf(
            whiteButton,
            blackButton,
            blueButton,
            greenButton,
            yellowButton,
            magentaButton,
            redButton,
            cyanButton
        )
        colorButtons.forEach { button ->
            button.setStrokeColorResource(R.color.blue)
        }

        selectNewTool(model.selectedTool)
        penSizeIndicator.text = drawingView.strokeState.penSizeMultiplier.toString()
    }

    private fun changeToolsVisibility(currentVisibility: Int) {
        val newVisibility = if (currentVisibility == View.VISIBLE) {
            View.GONE
        } else {
            View.VISIBLE
        }

        val onAnimationEnd = object : Animator.AnimatorListener {
            override fun onAnimationStart(animation: Animator?) {
                if (newVisibility == View.VISIBLE) {
                    toolbar.visibility = newVisibility
                }
            }

            override fun onAnimationEnd(animation: Animator?) {
                if (newVisibility == View.GONE) {
                    toolbar.visibility = newVisibility
                }
            }

            override fun onAnimationCancel(animation: Animator?) {}
            override fun onAnimationRepeat(animation: Animator?) {}
        }

        toolbar.animate().setDuration(300).alpha(if (newVisibility == View.VISIBLE) 1f else 0f)
            .setListener(onAnimationEnd).start()
    }

    private fun initializeDrawing() {
        val colorAliasInARGB =
            EnumMap(model.persistentDrawing.themeToARGBColors(resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK))

        val white = colorAliasInARGB[ColorAlias.White]
        val black = colorAliasInARGB[ColorAlias.Black]
        val red = colorAliasInARGB[ColorAlias.Red]
        val green = colorAliasInARGB[ColorAlias.Green]
        val cyan = colorAliasInARGB[ColorAlias.Cyan]
        val magenta = colorAliasInARGB[ColorAlias.Magenta]
        val blue = colorAliasInARGB[ColorAlias.Blue]
        val yellow = colorAliasInARGB[ColorAlias.Yellow]

        if (white == null || black == null || red == null || green == null || cyan == null || magenta == null || blue == null || yellow == null) {
            alertModel.notifyBasicError()
            return
        }

        drawingView.colorAliasInARGB = colorAliasInARGB

        whiteButton.backgroundTintList = ColorStateList(arrayOf(intArrayOf()), intArrayOf(white))
        blackButton.backgroundTintList = ColorStateList(arrayOf(intArrayOf()), intArrayOf(black))
        redButton.backgroundTintList = ColorStateList(arrayOf(intArrayOf()), intArrayOf(red))
        greenButton.backgroundTintList = ColorStateList(arrayOf(intArrayOf()), intArrayOf(green))
        cyanButton.backgroundTintList = ColorStateList(arrayOf(intArrayOf()), intArrayOf(cyan))
        magentaButton.backgroundTintList =
            ColorStateList(arrayOf(intArrayOf()), intArrayOf(magenta))
        blueButton.backgroundTintList = ColorStateList(arrayOf(intArrayOf()), intArrayOf(blue))
        yellowButton.backgroundTintList = ColorStateList(arrayOf(intArrayOf()), intArrayOf(yellow))

        toolbar.visibility = View.VISIBLE

        isFirstLaunch = false
        drawingView.initialize(model.persistentDrawing, model.persistentBitmap, model.persistentCanvas, model.persistentStrokeState)
    }

    @SuppressLint("ClickableViewAccessibility")
    private fun setUpToolbarListeners() {
        whiteButton.setOnClickListener {
            selectNewTool(DrawingView.Tool.Pen(ColorAlias.White))
        }

        blackButton.setOnClickListener {
            selectNewTool(DrawingView.Tool.Pen(ColorAlias.Black))
        }

        blueButton.setOnClickListener {
            selectNewTool(DrawingView.Tool.Pen(ColorAlias.Blue))
        }

        greenButton.setOnClickListener {
            selectNewTool(DrawingView.Tool.Pen(ColorAlias.Green))
        }

        yellowButton.setOnClickListener {
            selectNewTool(DrawingView.Tool.Pen(ColorAlias.Yellow))
        }

        magentaButton.setOnClickListener {
            selectNewTool(DrawingView.Tool.Pen(ColorAlias.Magenta))
        }

        redButton.setOnClickListener {
            selectNewTool(DrawingView.Tool.Pen(ColorAlias.Red))
        }

        cyanButton.setOnClickListener {
            selectNewTool(DrawingView.Tool.Pen(ColorAlias.Cyan))
        }

        eraser.setOnClickListener {
            selectNewTool(DrawingView.Tool.Eraser)
        }

        penSizeChooser.setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
            override fun onProgressChanged(seekBar: SeekBar?, progress: Int, fromUser: Boolean) {
                val adjustedProgress = progress + 1
                penSizeIndicator.text = adjustedProgress.toString()
                drawingView.strokeState.penSizeMultiplier = adjustedProgress
            }

            override fun onStartTrackingTouch(seekBar: SeekBar?) {}

            override fun onStopTrackingTouch(seekBar: SeekBar?) {}
        })

        gestureDetector = GestureDetector(
            requireContext(),
            object : GestureDetector.OnGestureListener {
                override fun onDown(e: MotionEvent?): Boolean = true

                override fun onShowPress(e: MotionEvent?) {}

                override fun onSingleTapUp(e: MotionEvent?): Boolean {
                    changeToolsVisibility(toolbar.visibility)
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
                            model.viewModelScope.launch(Dispatchers.IO) {
                                model.saveDrawing(
                                    drawingView.drawing.clone()
                                )
                            }
                        }
                    }
                }
            },
            1000,
            5000
        )
    }
}
