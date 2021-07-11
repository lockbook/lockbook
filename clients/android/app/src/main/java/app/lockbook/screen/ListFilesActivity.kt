package app.lockbook.screen

import android.content.Intent
import android.content.res.Configuration
import android.os.Bundle
import android.view.Menu
import android.view.MenuItem
import android.view.View
import androidx.appcompat.app.AppCompatActivity
import androidx.preference.PreferenceManager
import app.lockbook.R
import app.lockbook.databinding.ActivityListFilesBinding
import app.lockbook.model.AlertModel
import app.lockbook.util.*
import timber.log.Timber
import java.lang.ref.WeakReference

private val menuItemsNoneSelected = listOf(
    R.id.menu_list_files_sort,
    R.id.menu_list_files_file_layout,
)

private val menuItemsOneOrMoreSelected = listOf(
    R.id.menu_list_files_delete,
    R.id.menu_list_files_move,
    R.id.menu_list_files_share
)

private val menuItemsOneSelected = listOf(
    R.id.menu_list_files_rename,
    R.id.menu_list_files_info,
)

const val IS_THIS_AN_IMPORT = "is_this_an_import"

class ListFilesActivity : AppCompatActivity() {
    private var _binding: ActivityListFilesBinding? = null

    // This property is only valid between onCreateView and
    // onDestroyView.
    private val binding get() = _binding!!
    private var menu: Menu? = null

    private val alertModel by lazy {
        AlertModel(WeakReference(this))
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        _binding = ActivityListFilesBinding.inflate(layoutInflater)
        setContentView(binding.root)
        setSupportActionBar(binding.listFilesToolbar)
    }

    fun isThisAnImport(): Boolean {
        return intent.extras?.getBoolean(IS_THIS_AN_IMPORT, false) ?: false
    }

    override fun onCreateOptionsMenu(menu: Menu?): Boolean {
        menuInflater.inflate(R.menu.menu_list_files, menu)
        this.menu = menu
        setSelectedMenuOptions()

        val selectedFiles = getListFilesFragment()?.listFilesViewModel?.selectedFiles
        if (selectedFiles != null && selectedFiles.isNotEmpty()) {
            openFileMenu(selectedFiles)
        }

        return true
    }

    private fun setSelectedMenuOptions() {
        val preference = PreferenceManager.getDefaultSharedPreferences(application)

        when (
            val optionValue = preference.getString(
                getString(R.string.sort_files_key),
                getString(R.string.sort_files_a_z_value)
            )
        ) {
            getString(R.string.sort_files_a_z_value) -> menu?.findItem(R.id.menu_list_files_sort_a_z)?.isChecked = true
            getString(R.string.sort_files_z_a_value) -> menu?.findItem(R.id.menu_list_files_sort_z_a)?.isChecked = true
            getString(R.string.sort_files_last_changed_value) ->
                menu?.findItem(R.id.menu_list_files_sort_last_changed)?.isChecked =
                    true
            getString(R.string.sort_files_first_changed_value) ->
                menu?.findItem(R.id.menu_list_files_sort_first_changed)?.isChecked =
                    true
            getString(R.string.sort_files_type_value) -> menu?.findItem(R.id.menu_list_files_sort_type)?.isChecked = true
            else -> {
                Timber.e("File sorting shared preference does not match every supposed option: $optionValue")
                alertModel.notifyBasicError()
            }
        }.exhaustive

        val deviceConfig = resources.configuration

        when (
            val optionValue = preference.getString(
                getString(R.string.file_layout_key),
                if (deviceConfig.isLayoutSizeAtLeast(Configuration.SCREENLAYOUT_SIZE_LARGE) || (deviceConfig.screenWidthDp >= 480 && deviceConfig.screenHeightDp >= 640)) {
                    getString(R.string.file_layout_grid_value)
                } else {
                    getString(R.string.file_layout_linear_value)
                }
            )
        ) {
            getString(R.string.file_layout_linear_value) -> menu?.findItem(R.id.menu_list_files_linear_view)?.isChecked = true
            getString(R.string.file_layout_grid_value) -> menu?.findItem(R.id.menu_list_files_grid_view)?.isChecked = true
            else -> {
                Timber.e("File layout shared preference does not match every supposed option: $optionValue")
                alertModel.notifyBasicError()
            }
        }
    }

    override fun onOptionsItemSelected(item: MenuItem): Boolean {
        when (item.itemId) {
            R.id.menu_list_files_settings -> {
                startActivity(Intent(applicationContext, SettingsActivity::class.java))
            }
            R.id.menu_list_files_sort_last_changed,
            R.id.menu_list_files_sort_a_z,
            R.id.menu_list_files_sort_z_a,
            R.id.menu_list_files_sort_first_changed,
            R.id.menu_list_files_sort_type,
            R.id.menu_list_files_grid_view,
            R.id.menu_list_files_linear_view -> {
                menu?.findItem(item.itemId)?.isChecked = true
                getListFilesFragment()?.onMenuItemPressed(item.itemId)
            }
            R.id.menu_list_files_rename,
            R.id.menu_list_files_delete,
            R.id.menu_list_files_info,
            R.id.menu_list_files_move,
            R.id.menu_list_files_share -> {
                getListFilesFragment()?.onMenuItemPressed(item.itemId)
            }
            else -> return false
        }

        return true
    }

    fun switchMenu(expandOrNot: Boolean) {
        val fragment = getListFilesFragment() ?: return
        if (expandOrNot) {
            openFileMenu(fragment.listFilesViewModel.selectedFiles)
        } else {
            closeFileMenu()
        }
    }

    private fun openFileMenu(selected: List<ClientFileMetadata>) {
        for (menuItem in menuItemsNoneSelected) {
            menu?.findItem(menuItem)?.isVisible = false
        }

        for (menuItem in menuItemsOneOrMoreSelected) {
            menu?.findItem(menuItem)?.isVisible = true
        }

        if (selected.size == 1) {
            for (menuItem in menuItemsOneSelected) {
                menu?.findItem(menuItem)?.isVisible = true
            }
        } else {
            for (menuItem in menuItemsOneSelected) {
                menu?.findItem(menuItem)?.isVisible = false
            }
        }
    }

    private fun closeFileMenu() {
        for (menuItem in menuItemsOneOrMoreSelected) {
            menu?.findItem(menuItem)?.isVisible = false
        }

        for (menuItem in menuItemsOneSelected) {
            menu?.findItem(menuItem)?.isVisible = false
        }

        for (menuItem in menuItemsNoneSelected) {
            menu?.findItem(menuItem)?.isVisible = true
        }
    }

    private fun getListFilesFragment(): ListFilesFragment? {
        val fragments = supportFragmentManager.fragments
        val listFilesFragment = fragments[0]
        return if (listFilesFragment is ListFilesFragment) {
            listFilesFragment
        } else {
            Timber.e("Unable to retrieve ListFilesFragment.")
            alertModel.notifyBasicError()

            null
        }
    }

    fun showHideProgressOverlay(show: Boolean) {
        if (show) {
            Animate.animateVisibility(binding.progressOverlay.root, View.VISIBLE, 102, 500)
        } else {
            Animate.animateVisibility(binding.progressOverlay.root, View.GONE, 0, 500)
        }
    }

    override fun onBackPressed() {
        if (getListFilesFragment()?.onBackPressed() == false) {
            super.onBackPressed()
        }
    }
}
