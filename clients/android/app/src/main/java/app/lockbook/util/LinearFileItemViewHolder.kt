package app.lockbook.util

import android.view.View
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import app.lockbook.R
import com.afollestad.recyclical.ViewHolder

class LinearFileItemViewHolder(itemView: View) : ViewHolder(itemView) {
    val fileItemHolder: LinearLayout = itemView.findViewById(R.id.linear_file_item_holder)
    val name: TextView = itemView.findViewById(R.id.linear_file_name)
    val description: TextView = itemView.findViewById(R.id.linear_file_description)
    val icon: ImageView = itemView.findViewById(R.id.linear_file_icon)
}

class LinearMoveFileItemViewHolder(itemView: View) : ViewHolder(itemView) {
    val name: TextView = itemView.findViewById(R.id.linear_move_file_name)
    val icon: ImageView = itemView.findViewById(R.id.linear_move_file_icon)
}
