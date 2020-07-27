package app.lockbook.loggedin.settings

import android.content.Context
import android.util.Log
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.ArrayAdapter
import androidx.cardview.widget.CardView
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.loggedin.listfiles.ClickInterface
import kotlinx.android.synthetic.main.recyclerview_content_settings.view.*

class SettingsAdapter(settings: List<String>, val clickInterface: ClickInterface): RecyclerView.Adapter<SettingsAdapter.SettingsViewHolder>() {
    var settings = settings
        set(value) {
            field = value
            notifyDataSetChanged()
        }

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): SettingsViewHolder {
        val layoutInflater = LayoutInflater.from(parent.context)
        val view = layoutInflater.inflate(R.layout.recyclerview_content_settings, parent, false) as CardView

        return SettingsViewHolder(view)
    }

    override fun getItemCount(): Int = settings.size

    override fun onBindViewHolder(holder: SettingsViewHolder, position: Int) {
        val item = settings[position]

        holder.cardView.setting_title.text = item
    }

    inner class SettingsViewHolder(val cardView: CardView): RecyclerView.ViewHolder(cardView) {

        init {
            cardView.setOnClickListener {
                clickInterface.onItemClick(adapterPosition)
            }
        }
    }
}