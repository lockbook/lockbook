package app.lockbook.screen

import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.core.content.ContextCompat
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import com.google.android.material.listitem.ListItemCardView
import com.google.android.material.listitem.ListItemLayout
import com.google.android.material.textview.MaterialTextView

internal enum class FileSortCriterion {
    LastModified,
    Alphabetical,
}

internal enum class FileSortDirection {
    Ascending,
    Descending,
}

internal data class FileSortOptions(
    val criterion: FileSortCriterion = FileSortCriterion.LastModified,
    val direction: FileSortDirection = FileSortDirection.Descending,
)

internal class FileSortAdapter(
    private val onClick: (View) -> Unit,
) : RecyclerView.Adapter<FileSortAdapter.ViewHolder>() {
    private var options = FileSortOptions()
    private var totalItemCount = 1

    fun update(
        options: FileSortOptions,
        fileCount: Int,
    ) {
        this.options = options
        totalItemCount = fileCount + 1
        notifyItemChanged(0)
    }

    override fun onCreateViewHolder(
        parent: ViewGroup,
        viewType: Int,
    ): ViewHolder =
        ViewHolder(
            LayoutInflater.from(parent.context).inflate(R.layout.file_sort_item, parent, false),
            onClick,
        )

    override fun onBindViewHolder(
        holder: ViewHolder,
        position: Int,
    ) {
        holder.bind(options, totalItemCount)
    }

    override fun getItemCount(): Int = 1

    internal class ViewHolder(
        itemView: View,
        private val onClick: (View) -> Unit,
    ) : RecyclerView.ViewHolder(itemView) {
        private val holder: ListItemCardView = itemView.findViewById(R.id.file_sort_holder)
        private val label: MaterialTextView = itemView.findViewById(R.id.file_sort_label)
        private val arrow: View = itemView.findViewById(R.id.file_sort_arrow)

        init {
            holder.setOnClickListener(onClick)
            arrow.setOnClickListener(onClick)
        }

        fun bind(
            options: FileSortOptions,
            totalItemCount: Int,
        ) {
            label.setText(
                when (options.criterion) {
                    FileSortCriterion.LastModified -> R.string.sort_date_modified
                    FileSortCriterion.Alphabetical -> R.string.sort_alphabetical
                },
            )
            arrow.rotation = if (options.direction == FileSortDirection.Ascending) 180f else 0f
            arrow.contentDescription =
                itemView.context.getString(
                    if (options.direction == FileSortDirection.Ascending) {
                        R.string.sort_ascending
                    } else {
                        R.string.sort_descending
                    },
                )
            holder.setCardBackgroundColor(
                ContextCompat.getColorStateList(itemView.context, R.color.file_tree_list_item_background),
            )
            (itemView as ListItemLayout).updateAppearance(0, totalItemCount)
        }
    }
}
