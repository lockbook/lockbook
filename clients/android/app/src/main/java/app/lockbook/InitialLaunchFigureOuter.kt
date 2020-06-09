package app.lockbook

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import app.lockbook.core.isDbPresent
import app.lockbook.core.loadLockbookCore


class InitialLaunchFigureOuter : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        loadLockbookCore()

        if (isDbPresent(filesDir.absolutePath)) {
            val intent = Intent(this, ListFiles::class.java)
            startActivity(intent)
            finish()
        } else {
            val intent = Intent(this, Welcome::class.java)
            startActivity(intent)
            finish()
        }
    }
}