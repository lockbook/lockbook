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
import app.lockbook.model.OnFinishAlert
import app.lockbook.util.Animate
import app.lockbook.util.BASIC_ERROR
import app.lockbook.util.SharedPreferences.FILE_LAYOUT_KEY
import app.lockbook.util.SharedPreferences.GRID_LAYOUT
import app.lockbook.util.SharedPreferences.LINEAR_LAYOUT
import app.lockbook.util.SharedPreferences.SORT_FILES_A_Z
import app.lockbook.util.SharedPreferences.SORT_FILES_FIRST_CHANGED
import app.lockbook.util.SharedPreferences.SORT_FILES_KEY
import app.lockbook.util.SharedPreferences.SORT_FILES_LAST_CHANGED
import app.lockbook.util.SharedPreferences.SORT_FILES_TYPE
import app.lockbook.util.SharedPreferences.SORT_FILES_Z_A
import app.lockbook.util.exhaustive
import timber.log.Timber

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

class ListFilesActivity : AppCompatActivity() {
    private var _binding: ActivityListFilesBinding? = null

    // This property is only valid between onCreateView and
    // onDestroyView.
    private val binding get() = _binding!!
    private var menu: Menu? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        _binding = ActivityListFilesBinding.inflate(layoutInflater)
        setContentView(binding.root)
        setSupportActionBar(binding.listFilesToolbar)
    }

    override fun onCreateOptionsMenu(menu: Menu?): Boolean {
        menuInflater.inflate(R.menu.menu_list_files, menu)
        this.menu = menu
        setSelectedMenuOptions()

        val selectedFiles = getListFilesFragment()?.listFilesViewModel?.selectedFiles
        if (selectedFiles != null && selectedFiles.contains(true)) {
            openFileMenu(selectedFiles)
        }

        return true
    }

    private fun setSelectedMenuOptions() {
        val preference = PreferenceManager.getDefaultSharedPreferences(application)

        when (
            val optionValue = preference.getString(
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
                AlertModel.errorHasOccurred(
                    binding.listFilesActivityLayout,
                    BASIC_ERROR,
                    OnFinishAlert.DoNothingOnFinishAlert
                )
            }
        }.exhaustive

        val config = resources.configuration

        when (
            val optionValue = preference.getString(
                FILE_LAYOUT_KEY,
                if (config.isLayoutSizeAtLeast(Configuration.SCREENLAYOUT_SIZE_LARGE) || (config.screenWidthDp >= 480 && config.screenHeightDp >= 640)) {
                    GRID_LAYOUT
                } else {
                    LINEAR_LAYOUT
                }
            )
        ) {
            LINEAR_LAYOUT -> menu?.findItem(R.id.menu_list_files_linear_view)?.isChecked = true
            GRID_LAYOUT -> menu?.findItem(R.id.menu_list_files_grid_view)?.isChecked = true
            else -> {
                Timber.e("File layout shared preference does not match every supposed option: $optionValue")
                AlertModel.errorHasOccurred(
                    binding.listFilesActivityLayout,
                    BASIC_ERROR,
                    OnFinishAlert.DoNothingOnFinishAlert
                )
            }
        }
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
            R.id.menu_list_files_sort_type,
            R.id.menu_list_files_grid_view,
            R.id.menu_list_files_linear_view -> {
                menu?.findItem(item.itemId)?.isChecked = true
                getListFilesFragment()?.onMenuItemPressed(item.itemId)
                true
            }
            R.id.menu_list_files_rename,
            R.id.menu_list_files_delete,
            R.id.menu_list_files_info,
            R.id.menu_list_files_move,
            R.id.menu_list_files_share -> {
                getListFilesFragment()?.onMenuItemPressed(item.itemId)
                true
            }
            else -> false
        }.exhaustive
    }

    fun switchMenu() {
        val fragment = getListFilesFragment() ?: return
        if (fragment.listFilesViewModel.selectedFiles.contains(true)) {
            openFileMenu(fragment.listFilesViewModel.selectedFiles)
        } else {
            closeFileMenu()
        }
    }

    private fun openFileMenu(selected: List<Boolean>) {
        for (menuItem in menuItemsNoneSelected) {
            menu?.findItem(menuItem)?.isVisible = false
        }

        for (menuItem in menuItemsOneOrMoreSelected) {
            menu?.findItem(menuItem)?.isVisible = true
        }

        if (selected.filter { selectedFile -> selectedFile }.size == 1) {
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
            AlertModel.errorHasOccurred(
                binding.listFilesActivityLayout,
                BASIC_ERROR,
                OnFinishAlert.DoNothingOnFinishAlert
            )

            null
        }
    }

    fun showHideProgressOverlay(show: Boolean) {
        if (show) {
            Animate.animateVisibility(binding.progressOverlay.root, View.VISIBLE, 0.4f, 500)
        } else {
            Animate.animateVisibility(binding.progressOverlay.root, View.GONE, 0f, 500)
        }
    }

    override fun onBackPressed() {
        if (getListFilesFragment()?.onBackPressed() == false) {
            super.onBackPressed()
        }
    }
}
