package app.lockbook.screen

import android.view.View
import android.widget.ImageView
import android.widget.TextView
import androidx.annotation.StringRes
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.model.RecentPeriod

internal class DateSectionViewHolder(
    itemView: View,
) : RecyclerView.ViewHolder(itemView) {
    private val control: View = itemView.findViewById(R.id.date_section_control)
    private val title: TextView = itemView.findViewById(R.id.date_section_title)
    private val arrow: ImageView = itemView.findViewById(R.id.date_section_arrow)

    fun bind(
        period: RecentPeriod,
        collapsed: Boolean,
        onClick: () -> Unit,
    ) {
        title.setText(period.titleRes)
        arrow.rotation = if (collapsed) -90f else 0f
        control.contentDescription =
            control.context.getString(
                if (collapsed) R.string.expand_recent_group else R.string.collapse_recent_group,
                control.context.getString(period.titleRes),
            )
        control.setOnClickListener { onClick() }
    }
}

@get:StringRes
internal val RecentPeriod.titleRes: Int
    get() =
        when (this) {
            RecentPeriod.Today -> R.string.today
            RecentPeriod.Yesterday -> R.string.yesterday
            RecentPeriod.PreviousSevenDays -> R.string.previous_seven_days
            RecentPeriod.Older -> R.string.older
        }
