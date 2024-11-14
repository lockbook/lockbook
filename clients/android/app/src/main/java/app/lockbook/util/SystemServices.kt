package app.lockbook.util

import android.app.Application
import android.content.Context
import android.content.res.Resources
import android.view.View
import android.view.Window
import android.view.WindowManager
import android.view.inputmethod.InputMethodManager
import androidx.annotation.StringRes
import androidx.appcompat.app.AppCompatActivity
import androidx.fragment.app.Fragment
import androidx.lifecycle.AndroidViewModel
import androidx.navigation.fragment.NavHostFragment
import androidx.preference.Preference
import app.lockbook.App
import app.lockbook.R
import app.lockbook.screen.*

fun AndroidViewModel.getString(
    @StringRes stringRes: Int,
    vararg formatArgs: Any = emptyArray()
): String {
    return getString(this.getRes(), stringRes, *formatArgs)
}

fun AndroidViewModel.getContext(): Context {
    return this.getApplication<Application>()
}

fun AndroidViewModel.getRes(): Resources {
    return this.getApplication<Application>().resources
}

fun Window?.requestKeyboardFocus(view: View?) {
    this?.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_STATE_ALWAYS_VISIBLE)
    view?.requestFocus()
    (view?.context?.getSystemService(Context.INPUT_METHOD_SERVICE) as? InputMethodManager?)?.showSoftInput(view, InputMethodManager.SHOW_IMPLICIT)
}

fun Preference.getSettingsFragment(): SettingsFragment {
    return (context as SettingsActivity).supportFragmentManager.fragments[0] as SettingsFragment
}

fun Fragment.getSettingsActivity(): SettingsActivity {
    return (context as SettingsActivity)
}

fun AndroidViewModel.getApp(): App {
    return getApplication()
}

fun AppCompatActivity.getApp(): App {
    return application as App
}

fun Fragment.getApp(): App {
    return requireActivity().application as App
}

fun MainScreenActivity.navHost(): NavHostFragment =
    (supportFragmentManager.findFragmentById(R.id.files_container) as NavHostFragment)

fun MainScreenActivity.getFilesFragment(): FilesFragment =
    (supportFragmentManager.findFragmentById(R.id.files_container) as NavHostFragment).childFragmentManager.fragments[0] as FilesFragment

fun MainScreenActivity.maybeGetFilesFragment(): FilesFragment? =
    (supportFragmentManager.findFragmentById(R.id.files_container) as? NavHostFragment)?.childFragmentManager?.fragments?.get(0) as? FilesFragment

fun MainScreenActivity.maybeGetSearchFilesFragment(): SearchDocumentsFragment? =
    (supportFragmentManager.findFragmentById(R.id.files_container) as? NavHostFragment)?.childFragmentManager?.fragments?.get(0) as? SearchDocumentsFragment

fun SharesActivity.maybeGetCreateLinkFragment(): CreateLinkFragment? =
    (supportFragmentManager.findFragmentById(R.id.shared_files_container) as? NavHostFragment)?.childFragmentManager?.fragments?.get(0) as? CreateLinkFragment

fun getString(
    res: Resources,
    @StringRes stringRes: Int,
    vararg formatArgs: Any = emptyArray()
): String = res.getString(stringRes, *formatArgs)

class ExtensionHelper(val fileName: String) {
    private val extension: String
        get() {
            val indexOfDot = fileName.lastIndexOf('.')

            if (indexOfDot == -1) {
                return ""
            }

            return fileName.substring(indexOfDot + 1)
        }

    val isImage: Boolean
        get() = extension in setOf(
            "jpeg",
            "jpg",
            "png"
        )

    val isDrawing: Boolean get() = extension == "svg"

    val isPdf: Boolean get() = extension == "pdf"
}

val <T> T.exhaustive: T
    get() = this
