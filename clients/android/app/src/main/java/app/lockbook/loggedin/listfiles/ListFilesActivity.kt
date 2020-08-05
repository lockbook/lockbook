package app.lockbook.loggedin.listfiles

import android.content.Intent
import android.os.Bundle
import android.view.Menu
import android.view.MenuItem
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.R
import app.lockbook.loggedin.settings.SettingsActivity
import app.lockbook.utils.UNEXPECTED_ERROR_OCCURRED
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.Result
import timber.log.Timber

class ListFilesActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_list_files)
        Timber.plant(Timber.DebugTree())
    }

    override fun onCreateOptionsMenu(menu: Menu?): Boolean {
        menuInflater.inflate(R.menu.menu, menu)
        return true
    }

    override fun onOptionsItemSelected(item: MenuItem): Boolean {
        return when (item.itemId) {
            R.id.menu_settings -> {
                startActivity(Intent(applicationContext, SettingsActivity::class.java))
                true
            }
            R.id.menu_sort -> {
                val fragment = getFragment().component1()
                if (fragment is ListFilesFragment) {
                    fragment.onSortPressed()
                } else {
                    Timber.e("Unable to retrieve ListFilesFragment.")
                    Toast.makeText(this, UNEXPECTED_ERROR_OCCURRED, Toast.LENGTH_LONG).show()
                }
                true
            }
            else -> false
        }
    }

    private fun getFragment(): Result<ListFilesFragment, Unit> {
        val fragments = supportFragmentManager.fragments
        val listFilesFragment = fragments[0]
        if (listFilesFragment is ListFilesFragment) {
            return Ok(listFilesFragment)
        }

        return Err(Unit)
    }

    override fun onBackPressed() {
        when (getFragment().component1()?.onBackPressed()) {
            false -> super.onBackPressed()
            null -> {
                Timber.e("Unable to get result of back press.")
                Toast.makeText(this, UNEXPECTED_ERROR_OCCURRED, Toast.LENGTH_LONG).show()
            }
        }
    }
}
