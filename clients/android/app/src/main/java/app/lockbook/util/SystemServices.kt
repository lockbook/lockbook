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
import androidx.lifecycle.AndroidViewModel
import androidx.preference.Preference
import app.lockbook.App
import app.lockbook.screen.SettingsActivity
import app.lockbook.screen.SettingsFragment

fun AndroidViewModel.getString(
    @StringRes stringRes: Int,
    vararg formatArgs: Any = emptyArray()
): String {
    return app.lockbook.util.getString(this.getRes(), stringRes, *formatArgs)
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

fun AndroidViewModel.getApp(): App {
    return getApplication()
}

fun AppCompatActivity.getApp(): App {
    return application as App
}


