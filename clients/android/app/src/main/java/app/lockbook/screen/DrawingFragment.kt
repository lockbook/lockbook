package app.lockbook.screen

import android.animation.Animator
import android.annotation.SuppressLint
import android.content.res.Configuration
import android.os.Bundle
import android.view.*
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.fragment.app.viewModels
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import app.lockbook.R
import app.lockbook.databinding.FragmentDrawingBinding
import app.lockbook.model.*
import app.lockbook.ui.DrawingView
import app.lockbook.ui.PenState
import app.lockbook.util.ColorAlias
import app.lockbook.util.exhaustive
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
    private val fingerDrawing get() = binding.drawingToolbar.fingerDrawing
    private val penSizeChooser get() = binding.drawingToolbar.drawingPenSize

    private val toolbar get() = binding.drawingToolbar.drawingToolsMenu

    private val activityModel: StateViewModel by activityViewModels()
    private val drawingView get() = binding.drawingView
    private val model: DrawingViewModel by viewModels(
        factoryProducer = {
            object : ViewModelProvider.Factory {
                override fun <T : ViewModel> create(modelClass: Class<T>): T {
                    if (modelClass.isAssignableFrom(DrawingViewModel::class.java))
                        return DrawingViewModel(
                            requireActivity().application,
                            activityModel.detailsScreen!!.fileMetadata.id,
                            PersistentDrawingInfo(
                                drawing = (activityModel.detailsScreen as DetailsScreen.Drawing).drawing
                            )
                        ) as T
                    throw IllegalArgumentException("Unknown ViewModel class")
                }
            }
        }
    )

    private var isFirstLaunch = true
    private val gestureDetector by lazy {
        GestureDetector(
            requireContext(),
            object : GestureDetector.SimpleOnGestureListener() {
                override fun onSingleTapUp(e: MotionEvent): Boolean {
                    changeToolsVisibility(toolbar.visibility)
                    return true
                }
            }
        )
    }

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = FragmentDrawingBinding.inflate(inflater, container, false)

        initializeDrawing()
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
                drawingView.strokeState.penState = PenState.Drawing
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

                newButton.strokeWidth = 6
                drawingView.strokeState.strokeColor = newTool.colorAlias
            }
            is DrawingView.Tool.Eraser -> {
                eraser.setImageResource(R.drawable.ic_eraser_filled)
                drawingView.strokeState.penState = PenState.ErasingWithTouchButton
            }
        }.exhaustive

        model.selectedTool = newTool
    }

    private fun updateFingerDrawingButton() {
        if (model.isFingerDrawing) {
            fingerDrawing.setImageResource(R.drawable.ic_baseline_touch_app_24)
        } else {
            fingerDrawing.setImageResource(R.drawable.ic_outline_touch_app_24)
        }
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
            button.setStrokeColorResource(R.color.md_theme_primary)
        }

        selectNewTool(model.selectedTool)
        updateFingerDrawingButton()
    }

    private fun changeToolsVisibility(currentVisibility: Int) {
        val newVisibility = if (currentVisibility == View.VISIBLE) {
            View.GONE
        } else {
            View.VISIBLE
        }

        val onAnimationEnd = object : Animator.AnimatorListener {
            override fun onAnimationStart(p0: Animator) {
                if (newVisibility == View.VISIBLE) {
                    toolbar.visibility = newVisibility
                }
            }

            override fun onAnimationEnd(p0: Animator) {
                if (newVisibility == View.GONE) {
                    toolbar.visibility = newVisibility
                }
            }

            override fun onAnimationCancel(p0: Animator) {}

            override fun onAnimationRepeat(p0: Animator) {}
        }

        toolbar.animate().setDuration(300).alpha(if (newVisibility == View.VISIBLE) 1f else 0f)
            .setListener(onAnimationEnd).start()
    }

    private fun initializeDrawing() {
        val colorAliasInARGB =
            EnumMap(model.persistentDrawingInfo.drawing.themeToARGBColors(resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK))

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

        whiteButton.background.setTint(white)
        blackButton.background.setTint(black)
        redButton.background.setTint(red)
        greenButton.background.setTint(green)
        magentaButton.background.setTint(magenta)
        blueButton.background.setTint(blue)
        yellowButton.background.setTint(yellow)
        cyanButton.background.setTint(cyan)

        toolbar.visibility = View.VISIBLE

        isFirstLaunch = false
        drawingView.initialize(model.persistentDrawingInfo)
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

        fingerDrawing.setOnClickListener {
            model.isFingerDrawing = !model.isFingerDrawing
            updateFingerDrawingButton()
        }

        penSizeChooser?.addOnChangeListener { _, value, _ ->
            drawingView.strokeState.penSizeMultiplier = value.toInt()
        }

        drawingView.setOnTouchListener { _, event ->
            if (event != null && event.getToolType(0) == MotionEvent.TOOL_TYPE_FINGER) {
                gestureDetector.onTouchEvent(event)
            }

            false
        }
    }

    fun saveOnExit() {
        if (model.persistentDrawingInfo.drawing.isDirty) {
            model.lastEdit = System.currentTimeMillis()
            activityModel.saveDrawingOnExit(model.id, model.persistentDrawingInfo.drawing)
        }
    }
}
