package app.lockbook

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import app.lockbook.core.isDbPresent
import app.lockbook.core.loadLockbookCore
import app.lockbook.listfiles.ListFilesActivity


class InitialLaunchFigureOuter : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        loadLockbookCore()

        if (isDbPresent(filesDir.absolutePath)) {
            val intent = Intent(this, ListFilesActivity::class.java)
            startActivity(intent)
            finish()
        } else {
            val intent = Intent(this, WelcomeActivity::class.java)
            startActivity(intent)
            finish()
        }
    }
}