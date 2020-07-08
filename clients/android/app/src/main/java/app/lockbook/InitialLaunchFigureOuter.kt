package app.lockbook

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import app.lockbook.core.isDbPresent
import app.lockbook.core.loadLockbookCore
import app.lockbook.login.WelcomeActivity
import app.lockbook.loggedin.mainscreen.MainScreenActivity


class InitialLaunchFigureOuter : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        loadLockbookCore()

        if (isDbPresent(filesDir.absolutePath)) {
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