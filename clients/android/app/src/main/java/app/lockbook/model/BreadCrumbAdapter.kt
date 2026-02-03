package app.lockbook.model

import android.annotation.SuppressLint
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.ImageView
import android.widget.TextView
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.ui.BreadCrumbItem
import com.google.android.material.button.MaterialButton
import net.lockbook.File

@SuppressLint("NotifyDataSetChanged")
class BreadCrumbAdapter(var breadCrumbItemClickListener: BreadCrumbItemClickListener) : RecyclerView.Adapter<BreadCrumbAdapter.ViewHolder>() {

    private var breadCrumbItemsData: MutableList<BreadCrumbItem> = mutableListOf()
    private var arrowDrawable: Int = R.drawable.ic_baseline_keyboard_arrow_right_24
    private var textColor: Int = 10
    private var textSize: Int = 10

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): ViewHolder {
        return ViewHolder(LayoutInflater.from(parent.context).inflate(R.layout.bread_crumb_item, parent, false))
    }

    override fun getItemCount(): Int = breadCrumbItemsData.size

    override fun onBindViewHolder(holder: ViewHolder, position: Int) {
        val item = breadCrumbItemsData[position]

        if (position == 0) {
            holder.breadCrumbSeparator.visibility = View.GONE
        } else {
            holder.breadCrumbSeparator.visibility = View.VISIBLE
        }

        holder.breadCrumbTitle.text = item.file.name
    }

    fun setBreadCrumbItems(items: MutableList<BreadCrumbItem>) {
        breadCrumbItemsData = items
        notifyDataSetChanged()
    }

    fun setArrowDrawable(arrowDrawable: Int) {
        this.arrowDrawable = arrowDrawable
        notifyDataSetChanged()
    }

    fun setTextColor(textColor: Int) {
        this.textColor = textColor
        notifyDataSetChanged()
    }

    fun setTextSize(textSize: Int) {
        this.textSize = textSize
        notifyDataSetChanged()
    }

    inner class ViewHolder(breadCrumbItem: View) : RecyclerView.ViewHolder(breadCrumbItem) {
        var breadCrumbTitle: MaterialButton = itemView.findViewById(R.id.bread_crumb_title)
        var breadCrumbSeparator: ImageView = itemView.findViewById(R.id.bread_crumb_separator)

        init {
            breadCrumbTitle.setOnClickListener { view ->
                val file = breadCrumbItemsData[adapterPosition].file
                breadCrumbItemClickListener.onItemClick(view, file)
            }

            breadCrumbSeparator.setImageResource(arrowDrawable)
            breadCrumbTitle.textSize = textSize.toFloat()
        }
    }
}

interface BreadCrumbItemClickListener {
    fun onItemClick(breadCrumbItem: View, file: File)
}
