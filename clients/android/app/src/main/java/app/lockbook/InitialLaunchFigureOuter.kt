package app.lockbook

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.util.Log
import app.lockbook.core.loadLockbookCore
import app.lockbook.login.WelcomeActivity
import app.lockbook.loggedin.mainscreen.MainScreenActivity


class InitialLaunchFigureOuter : Activity() {

    companion object {
        const val SHARED_PREF_FILE = "app.lockbook"
        const val KEY = "loggedin"
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        loadLockbookCore()

        val pref = getSharedPreferences(SHARED_PREF_FILE, Context.MODE_PRIVATE);

        if (pref.getBoolean(KEY, false)) {
            val intent = Intent(this, MainScreenActivity::class.java)
            startActivity(intent)
            finish()
        } else {
            val intent = Intent(this, WelcomeActivity::class.java)
            startActivity(intent)
            finish()
        }
    }

}