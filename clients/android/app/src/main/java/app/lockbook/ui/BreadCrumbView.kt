package app.lockbook.ui

import android.content.Context
import android.util.AttributeSet
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.model.BreadCrumbAdapter
import app.lockbook.model.BreadCrumbItemClickListener
import net.lockbook.File

data class BreadCrumbItem(
    val file: File
)

class BreadCrumbView : FrameLayout {

    private lateinit var recyclerView: RecyclerView
    private lateinit var breadCrumbAdapter: BreadCrumbAdapter

    constructor(context: Context) : this(context, null)
    constructor(context: Context, attrs: AttributeSet?) : this(context, attrs, 0)
    constructor(context: Context, attrs: AttributeSet?, defStyleAttr: Int) : super(
        context,
        attrs,
        defStyleAttr
    ) {
        createAndAddRecyclerView(context)

        attrs?.let {
            val typedArray =
                context.obtainStyledAttributes(attrs, R.styleable.BreadCrumbView, defStyleAttr, 0)
            val arrowDrawable =
                typedArray.getResourceId(R.styleable.BreadCrumbView_arrow_drawable, -1)
            val textColor = typedArray.getColor(R.styleable.BreadCrumbView_text_color, -1)
            val textSize = typedArray.getColor(R.styleable.BreadCrumbView_text_size, -1)
            typedArray.recycle()
            if (arrowDrawable != -1) {
                breadCrumbAdapter.setArrowDrawable(arrowDrawable)
            }
            if (textColor != -1) {
                breadCrumbAdapter.setTextColor(textColor)
            }
            if (textSize != -1) {
                breadCrumbAdapter.setTextSize(textSize)
            }
        }

    }

    private fun createAndAddRecyclerView(context: Context) {
        recyclerView = RecyclerView(context)
        val recyclerViewParams = ViewGroup.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.WRAP_CONTENT
        )

        recyclerView.layoutManager =
            LinearLayoutManager(context, LinearLayoutManager.HORIZONTAL, false)
        breadCrumbAdapter = BreadCrumbAdapter(object : BreadCrumbItemClickListener {
            override fun onItemClick(breadCrumbItem: View, file: File) {}
        })

        recyclerView.adapter = breadCrumbAdapter
        addView(recyclerView, recyclerViewParams)
    }

    fun setListener(listener: BreadCrumbItemClickListener) {
        breadCrumbAdapter.breadCrumbItemClickListener = listener
    }

    fun setBreadCrumbItems(items: MutableList<BreadCrumbItem>) {
        breadCrumbAdapter.setBreadCrumbItems(items)
    }
}
