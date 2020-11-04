package app.lockbook.screen

import android.content.Intent
import android.os.Bundle
import android.view.Menu
import android.view.MenuItem
import androidx.appcompat.app.AppCompatActivity
import androidx.preference.PreferenceManager
import app.lockbook.R
import app.lockbook.util.Messages.UNEXPECTED_CLIENT_ERROR
import app.lockbook.util.SharedPreferences.SORT_FILES_A_Z
import app.lockbook.util.SharedPreferences.SORT_FILES_FIRST_CHANGED
import app.lockbook.util.SharedPreferences.SORT_FILES_KEY
import app.lockbook.util.SharedPreferences.SORT_FILES_LAST_CHANGED
import app.lockbook.util.SharedPreferences.SORT_FILES_TYPE
import app.lockbook.util.SharedPreferences.SORT_FILES_Z_A
import app.lockbook.util.exhaustive
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.Result
import com.google.android.material.snackbar.Snackbar
import kotlinx.android.synthetic.main.activity_list_files.*
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

        val fragment = getFragment().component1()
        if (fragment is ListFilesFragment) {
            if (fragment.listFilesViewModel.fileMenuShowing) {
                openFileMenu()
            }
        } else {
            Timber.e("Unable to retrieve ListFilesFragment.")
            Snackbar.make(list_files_activity_layout, UNEXPECTED_CLIENT_ERROR, Snackbar.LENGTH_SHORT).show()
        }

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
                    list_files_activity_layout,
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
            R.id.menu_list_files_rename -> {
                val fragment = getFragment().component1()
                if (fragment is ListFilesFragment) {
                    fragment.initiateRenameFileDialog()
                } else {
                    Timber.e("Unable to retrieve ListFilesFragment.")
                    Snackbar.make(list_files_activity_layout, UNEXPECTED_CLIENT_ERROR, Snackbar.LENGTH_SHORT).show()
                }
                true
            }
            R.id.menu_list_files_delete -> {
                val fragment = getFragment().component1()
                if (fragment is ListFilesFragment) {
                    fragment.onSortPressed(item.itemId)
                } else {
                    Timber.e("Unable to retrieve ListFilesFragment.")
                    Snackbar.make(list_files_activity_layout, UNEXPECTED_CLIENT_ERROR, Snackbar.LENGTH_SHORT).show()
                }
                true
            }
            R.id.menu_list_files_info -> {
                val fragment = getFragment().component1()
                if (fragment is ListFilesFragment) {
                    fragment.showMoreInfoDialog()
                } else {
                    Timber.e("Unable to retrieve ListFilesFragment.")
                    Snackbar.make(list_files_activity_layout, UNEXPECTED_CLIENT_ERROR, Snackbar.LENGTH_SHORT).show()
                }
                true
            }
            R.id.menu_list_files_move -> {
                val fragment = getFragment().component1()
                if (fragment is ListFilesFragment) {
                    fragment.moveSelectedFiles()
                } else {
                    Timber.e("Unable to retrieve ListFilesFragment.")
                    Snackbar.make(list_files_activity_layout, UNEXPECTED_CLIENT_ERROR, Snackbar.LENGTH_SHORT).show()
                }
                true
            }
            else -> false
        }.exhaustive
    }

    fun switchMenu() {
        val fragment = getFragment().component1()
        if (fragment is ListFilesFragment) {
            if (fragment.listFilesViewModel.fileMenuShowing) {
                menu?.findItem(R.id.menu_list_files_rename)?.isVisible = false
                menu?.findItem(R.id.menu_list_files_delete)?.isVisible = false
                menu?.findItem(R.id.menu_list_files_info)?.isVisible = false
                menu?.findItem(R.id.menu_list_files_move)?.isVisible = false
                menu?.findItem(R.id.menu_list_files_sort)?.isVisible = true
            } else {
                menu?.findItem(R.id.menu_list_files_rename)?.isVisible = true
                menu?.findItem(R.id.menu_list_files_delete)?.isVisible = true
                menu?.findItem(R.id.menu_list_files_info)?.isVisible = true
                menu?.findItem(R.id.menu_list_files_move)?.isVisible = true
                menu?.findItem(R.id.menu_list_files_sort)?.isVisible = false
            }

            fragment.listFilesViewModel.fileMenuShowing = !fragment.listFilesViewModel.fileMenuShowing
        } else {
            Timber.e("Unable to retrieve ListFilesFragment.")
            Snackbar.make(list_files_activity_layout, UNEXPECTED_CLIENT_ERROR, Snackbar.LENGTH_SHORT).show()
        }
    }

    private fun openFileMenu() {
        menu?.findItem(R.id.menu_list_files_rename)?.isVisible = true
        menu?.findItem(R.id.menu_list_files_delete)?.isVisible = true
        menu?.findItem(R.id.menu_list_files_info)?.isVisible = true
        menu?.findItem(R.id.menu_list_files_move)?.isVisible = true
        menu?.findItem(R.id.menu_list_files_sort)?.isVisible = false
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
