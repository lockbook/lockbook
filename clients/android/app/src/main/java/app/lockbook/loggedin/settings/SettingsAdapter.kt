package app.lockbook.loggedin.settings

import android.content.Context
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.ArrayAdapter
import app.lockbook.R
import kotlinx.android.synthetic.main.listview_content_settings.view.*

class SettingsAdapter(private val settingsList: List<String>, context: Context): ArrayAdapter<String>(context, settingsList.size) {

    private val layoutInflater = LayoutInflater.from(context)

    override fun getView(position: Int, convertView: View?, parent: ViewGroup): View {
        val itemView = layoutInflater.inflate(R.layout.listview_content_settings, parent, false)
        itemView.setting_title.text = settingsList[position]

        return itemView
    }
}