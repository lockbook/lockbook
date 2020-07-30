package app.lockbook.loggedin.settings

import android.view.LayoutInflater
import android.view.ViewGroup
import androidx.cardview.widget.CardView
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.utils.ClickInterface
import kotlinx.android.synthetic.main.recyclerview_content_settings.view.*

class SettingsAdapter(settings: List<String>, val clickInterface: ClickInterface, private val biometricAvailable: Boolean) : RecyclerView.Adapter<SettingsAdapter.SettingsViewHolder>() {
    private var settings = settings
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

    inner class SettingsViewHolder(val cardView: CardView) : RecyclerView.ViewHolder(cardView) {

        init {
            cardView.setOnClickListener {
                clickInterface.onItemClick(adapterPosition)
            }
        }
    }
}
