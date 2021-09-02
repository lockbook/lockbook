package app.lockbook.util

import android.view.View
import android.widget.ImageView
import android.widget.TextView
import app.lockbook.R
import com.afollestad.recyclical.ViewHolder

class HorizontalViewHolder(itemView: View) : ViewHolder(itemView) {
    val name: TextView = itemView.findViewById(R.id.linear_file_name)
    val description: TextView = itemView.findViewById(R.id.linear_file_description)
    val icon: ImageView = itemView.findViewById(R.id.linear_file_icon)
}