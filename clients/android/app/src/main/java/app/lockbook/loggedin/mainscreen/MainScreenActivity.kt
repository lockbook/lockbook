package app.lockbook.loggedin.mainscreen

import android.content.Intent
import android.os.Bundle
import android.view.Menu
import android.view.MenuItem
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.R
import app.lockbook.loggedin.settings.SettingsActivity

class MainScreenActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main_screen)
    }

    override fun onCreateOptionsMenu(menu: Menu?): Boolean {
        menuInflater.inflate(R.menu.menu_main_screen, menu)
        return true
    }

    override fun onOptionsItemSelected(item: MenuItem): Boolean {
        return when(item.itemId) {
            R.id.settings -> {
                startActivity(Intent(applicationContext, SettingsActivity::class.java))
                true
            }
            else -> false
        }
    }

    override fun onBackPressed() {
        val fragments = supportFragmentManager.fragments

        for(fragment in fragments) {
            if(fragment is MainScreenFragment) {
                if(!fragment.onBackPressed()) {
                    super.onBackPressed()
                }
            }
        }
    }
}
