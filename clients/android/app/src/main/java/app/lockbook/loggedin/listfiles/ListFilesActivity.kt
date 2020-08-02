package app.lockbook.loggedin.listfiles

import android.content.Intent
import android.os.Bundle
import android.view.Menu
import android.view.MenuItem
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.R
import app.lockbook.loggedin.settings.SettingsActivity

class ListFilesActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_list_files)
    }

    override fun onCreateOptionsMenu(menu: Menu?): Boolean {
        menuInflater.inflate(R.menu.menu_main_screen, menu)
        return true
    }

    override fun onOptionsItemSelected(item: MenuItem): Boolean {
        return when (item.itemId) {
            R.id.settings -> {
                startActivity(Intent(applicationContext, SettingsActivity::class.java))
                true
            }
            else -> false
        }
    }

    override fun onBackPressed() {
        val fragments = supportFragmentManager.fragments

        for (fragment in fragments) { // maybe do fragments[0] cause there is only 1
            if (fragment is ListFilesFragment) {
                if (!fragment.onBackPressed()) {
                    super.onBackPressed()
                }
            }
        }
    }
}
