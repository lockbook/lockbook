package app.lockbook

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.os.Bundle
import app.lockbook.core.loadLockbookCore
import app.lockbook.login.WelcomeActivity
import app.lockbook.loggedin.mainscreen.MainScreenActivity
import app.lockbook.utils.SharedPreferences.BIOMETRIC_NONE
import app.lockbook.utils.SharedPreferences.BIOMETRIC_RECOMMENDED
import app.lockbook.utils.SharedPreferences.BIOMETRIC_STRICT
import app.lockbook.utils.SharedPreferences.LOGGED_IN_KEY
import app.lockbook.utils.SharedPreferences.SHARED_PREF_FILE

class InitialLaunchFigureOuter : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        loadLockbookCore()

        val pref = getSharedPreferences(SHARED_PREF_FILE, Context.MODE_PRIVATE)

        if (pref.getBoolean(LOGGED_IN_KEY, false)) {
            when(pref.getInt("BIOMETRIC_OPTION_KEY", BIOMETRIC_RECOMMENDED)) {
                BIOMETRIC_NONE -> {}
                BIOMETRIC_RECOMMENDED -> {}
                BIOMETRIC_STRICT -> {}
            }
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