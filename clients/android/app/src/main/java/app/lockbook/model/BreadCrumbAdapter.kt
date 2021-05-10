package app.lockbook.model

import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.ImageView
import android.widget.TextView
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.ui.BreadCrumbItem

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

        holder.breadCrumbTitle.text = item.title
    }

    fun getBreadCrumbItem(position: Int) = breadCrumbItemsData[position]

    fun getBreadCrumbItemsSize(): Int = breadCrumbItemsData.size

    fun removeLastBreadCrumbItem() {
        breadCrumbItemsData.removeLast()
        notifyDataSetChanged()
    }

    fun removeAllBreadCrumbItems() {
        breadCrumbItemsData.removeAll { true }
        notifyDataSetChanged()
    }

    fun addBreadCrumbItem(item: BreadCrumbItem) {
        breadCrumbItemsData.add(item)
        notifyDataSetChanged()
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
        var breadCrumbTitle: TextView = itemView.findViewById(R.id.bread_crumb_title)
        var breadCrumbSeparator: ImageView = itemView.findViewById(R.id.bread_crumb_separator)

        init {
            breadCrumbTitle.setOnClickListener { view ->
                breadCrumbItemClickListener.onItemClick(view, adapterPosition)
            }

            breadCrumbSeparator.setImageResource(arrowDrawable)
            breadCrumbTitle.setTextColor(textColor)
            breadCrumbTitle.textSize = textSize.toFloat()
        }
    }
}

interface BreadCrumbItemClickListener {
    fun onItemClick(breadCrumbItem: View, position: Int)
}
