package app.lockbook.loggedin.listfiles

import android.content.Intent
import android.os.Bundle
import android.text.SpannableString
import android.text.style.ForegroundColorSpan
import android.view.Menu
import android.view.MenuItem
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.res.ResourcesCompat
import androidx.preference.PreferenceManager
import app.lockbook.R
import app.lockbook.loggedin.settings.SettingsActivity
import app.lockbook.utils.Messages.UNEXPECTED_CLIENT_ERROR
import app.lockbook.utils.SharedPreferences.SORT_FILES_A_Z
import app.lockbook.utils.SharedPreferences.SORT_FILES_FIRST_CHANGED
import app.lockbook.utils.SharedPreferences.SORT_FILES_KEY
import app.lockbook.utils.SharedPreferences.SORT_FILES_LAST_CHANGED
import app.lockbook.utils.SharedPreferences.SORT_FILES_TYPE
import app.lockbook.utils.SharedPreferences.SORT_FILES_Z_A
import app.lockbook.utils.exhaustive
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.Result
import com.google.android.material.snackbar.Snackbar
import kotlinx.android.synthetic.main.activity_list_files.*
import kotlinx.android.synthetic.main.splash_screen.*
import timber.log.Timber

class ListFilesActivity : AppCompatActivity() {
    private var menu: Menu? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_list_files)

        list_files_toolbar.title = "Lockbook"
        setSupportActionBar(list_files_toolbar)
    }

    override fun onCreateOptionsMenu(menu: Menu?): Boolean {
        menuInflater.inflate(R.menu.menu_list_files, menu)
        this.menu = menu
        matchToDefaultSortOption()
        return true
    }

    private fun matchToDefaultSortOption() {
        when (
            val optionValue = PreferenceManager.getDefaultSharedPreferences(application).getString(
                SORT_FILES_KEY,
                SORT_FILES_A_Z
            )
        ) {
            SORT_FILES_A_Z -> menu?.findItem(R.id.menu_list_files_sort_a_z)?.isChecked = true
            SORT_FILES_Z_A -> menu?.findItem(R.id.menu_list_files_sort_z_a)?.isChecked = true
            SORT_FILES_LAST_CHANGED ->
                menu?.findItem(R.id.menu_list_files_sort_last_changed)?.isChecked =
                    true
            SORT_FILES_FIRST_CHANGED ->
                menu?.findItem(R.id.menu_list_files_sort_first_changed)?.isChecked =
                    true
            SORT_FILES_TYPE -> menu?.findItem(R.id.menu_list_files_sort_type)?.isChecked = true
            else -> {
                Timber.e("File sorting shared preference does not match every supposed option: $optionValue")
                Snackbar.make(
                    splash_screen,
                    UNEXPECTED_CLIENT_ERROR,
                    Snackbar.LENGTH_SHORT
                ).show()
            }
        }.exhaustive
    }

    override fun onOptionsItemSelected(item: MenuItem): Boolean {
        return when (item.itemId) {
            R.id.menu_list_files_settings -> {
                startActivity(Intent(applicationContext, SettingsActivity::class.java))
                true
            }
            R.id.menu_list_files_sort_last_changed,
            R.id.menu_list_files_sort_a_z,
            R.id.menu_list_files_sort_z_a,
            R.id.menu_list_files_sort_first_changed,
            R.id.menu_list_files_sort_type -> {
                menu?.findItem(item.itemId)?.isChecked = true
                val fragment = getFragment().component1()
                if (fragment is ListFilesFragment) {
                    fragment.onSortPressed(item.itemId)
                } else {
                    Timber.e("Unable to retrieve ListFilesFragment.")
                    Snackbar.make(list_files_activity_layout, UNEXPECTED_CLIENT_ERROR, Snackbar.LENGTH_SHORT).show()
                }
                true
            }
            else -> false
        }.exhaustive
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
            true -> {
            }
            null -> {
                Timber.e("Unable to get result of back press.")
                Snackbar.make(list_files_activity_layout, UNEXPECTED_CLIENT_ERROR, Snackbar.LENGTH_SHORT).show()
            }
        }.exhaustive
    }
}
