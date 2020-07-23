package app.lockbook.loggedin.settings

import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity
import androidx.databinding.DataBindingUtil
import androidx.lifecycle.ViewModelProvider
import androidx.recyclerview.widget.LinearLayoutManager
import app.lockbook.R
import app.lockbook.databinding.ActivitySettingsBinding
import app.lockbook.loggedin.listfiles.FilesFoldersAdapter

class SettingsActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val binding: ActivitySettingsBinding = DataBindingUtil.setContentView(
            this,
            R.layout.activity_settings
        )

        val settingsViewModelFactory =
            SettingsViewModelFactory()
        val settingsViewModel =
            ViewModelProvider(this, settingsViewModelFactory).get(SettingsViewModel::class.java)
        val adapter = FilesFoldersAdapter(settingsViewModel)

        binding.settingsViewModel = settingsViewModel
        binding.settingsList.adapter = adapter
        binding.settingsList.layoutManager = LinearLayoutManager(applicationContext)



    }
}
