package app.lockbook.mainscreen

import android.content.Intent
import android.os.Bundle
import android.os.Handler
import android.view.Menu
import android.view.MenuItem
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.R
import kotlin.system.exitProcess

class MainScreenActivity : AppCompatActivity() {

    private var exit = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_list_files)
    }

    override fun onCreateOptionsMenu(menu: Menu?): Boolean {
        menuInflater.inflate(R.menu.list_files_menu, menu)
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
        if(exit) {
            finish()
            exitProcess(0)
        } else {
            Toast.makeText(this, "Press back again to exit.", Toast.LENGTH_SHORT).show()
            exit = true
            Handler().postDelayed({
                exit = false
            }, 2500)
        }
    }
}
