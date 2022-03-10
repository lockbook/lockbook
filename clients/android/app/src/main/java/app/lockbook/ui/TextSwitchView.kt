package app.lockbook.ui

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.drawable.TransitionDrawable
import android.util.AttributeSet
import android.view.View
import android.widget.Button
import android.widget.LinearLayout
import androidx.appcompat.content.res.AppCompatResources
import androidx.appcompat.widget.SwitchCompat
import app.lockbook.R
import app.lockbook.util.Animate
import app.lockbook.util.exhaustive

class TextSwitchView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
    defStyleAttr: Int = 0
) : LinearLayout(context, attrs, defStyleAttr) {
    private val styledAttrs = context.obtainStyledAttributes(attrs, R.styleable.TextSwitchView)

    private val optionOneText = styledAttrs.getText(R.styleable.TextSwitchView_tsv_option_one_text)?.toString() ?: "Not set"
    private val optionTwoText = styledAttrs.getText(R.styleable.TextSwitchView_tsv_option_two_text)?.toString() ?: "Not set"
    private var isOptionOneChosen = styledAttrs.getBoolean(R.styleable.TextSwitchView_tsv_is_option_one_chosen, false)
    private var textSize = styledAttrs.getFloat(R.styleable.TextSwitchView_tsv_is_option_one_chosen, -1f)

    private val view = View.inflate(context, R.layout.text_switch, this)
    private val optionOneButton = view.findViewById<Button>(R.id.tsv_option_one)
    private val optionTwoButton = view.findViewById<Button>(R.id.tsv_option_two)

    private var textSwitchListener: TextSwitchListener? = null

    init {
        optionOneButton.text = optionOneText
        optionTwoButton.text = optionTwoText

        if(isOptionOneChosen) {
            optionOneButton.background = AppCompatResources.getDrawable(context, R.drawable.text_switch_bg)
        } else {
            optionTwoButton.background = AppCompatResources.getDrawable(context, R.drawable.text_switch_bg)
        }

        optionOneButton.setOnClickListener {
            if(!isOptionOneChosen) {
                optionTwoButton.background = null
                optionOneButton.background = AppCompatResources.getDrawable(context, R.drawable.text_switch_bg)
                isOptionOneChosen = true
            }
        }

        optionTwoButton.setOnClickListener {
            if(isOptionOneChosen) {
                optionOneButton.background = null
                optionTwoButton.background = AppCompatResources.getDrawable(context, R.drawable.text_switch_bg)
                isOptionOneChosen = false
            }
        }

        if(textSize != -1f) {
            optionOneButton.textSize = textSize
            optionTwoButton.textSize = textSize
        }

        styledAttrs.recycle()
    }

    private fun animateButton(option: TextSwitchOption) {
        val (newOption, oldOption) = when(option) {
            TextSwitchOption.One -> {
                listOf(optionOneButton, optionTwoButton)
            }
            TextSwitchOption.Two -> {
                listOf(optionTwoButton, optionOneButton)
            }
        }
    }

    fun addSwitchListener(listener: TextSwitchListener) {
        textSwitchListener = listener
    }

//    override fun onDraw(canvas: Canvas?) {
//        paint.color = widgetBackgroundColor
////        canvas!!.drawCircle((measuredWidth / 2f), (measuredHeight / 2f), 5f, paint)
//        canvas!!.drawRoundRect(0f, 0f, measuredWidth.toFloat(), measuredHeight.toFloat(), 50f, 50f, paint)
//
//        paint.color = textsColor
//        paint.textSize = textsSize
//        paint.textAlign = Paint.Align.LEFT
//
//        val baseline = measuredHeight / 2f + (textsSize / 2f)
//        val sidePad = (measuredWidth / 9f)
//
//        canvas.drawText(onText, sidePad, baseline, paint)
//
//        paint.textAlign = Paint.Align.RIGHT
//
//        canvas.drawText(offText, measuredWidth - sidePad, baseline, paint)
//    }
//
//    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
//        val widthMode = MeasureSpec.getMode(widthMeasureSpec)
//        val widthSize = MeasureSpec.getSize(widthMeasureSpec)
//        val heightMode = MeasureSpec.getMode(heightMeasureSpec)
//        val heightSize = MeasureSpec.getSize(heightMeasureSpec)
//
//        val width = if (widthMode == MeasureSpec.EXACTLY) {
//            widthSize
//        } else if (widthMode == MeasureSpec.AT_MOST) {
//            Math.min(WIDTH, widthSize)
//        } else {
//            WIDTH
//        }
//
//        resolveSize()
//
//        val height = if (heightMode == MeasureSpec.EXACTLY) {
//            heightSize
//        } else if (heightMode == MeasureSpec.AT_MOST) {
//            Math.min(HEIGHT, heightSize)
//        } else {
//            HEIGHT
//        }
//
//        setMeasuredDimension(width, height)
//    }

    companion object {
        const val WIDTH: Int = 500
        const val HEIGHT: Int = 200
    }
}

interface TextSwitchListener {
    fun onSwitchClicked(option: TextSwitchOption)
}

enum class TextSwitchOption {
    One,
    Two
}