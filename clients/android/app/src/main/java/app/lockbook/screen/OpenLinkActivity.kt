package app.lockbook.screen

import android.content.Intent
import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.R
import app.lockbook.util.OpenLinkParser
import app.lockbook.util.PendingOpenLinkStore
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import net.lockbook.Lb
import net.lockbook.LbError

class OpenLinkActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        route(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        route(intent)
    }

    private fun route(intent: Intent) {
        val request = intent.dataString?.let(OpenLinkParser::parse)
        if (request == null) {
            MaterialAlertDialogBuilder(this)
                .setMessage(R.string.invalid_lockbook_link)
                .setPositiveButton(android.R.string.ok) { _, _ -> finish() }
                .setOnCancelListener { finish() }
                .show()
            return
        }

        try {
            Lb.getAccount()
        } catch (_: LbError) {
            finish()
            return
        }

        PendingOpenLinkStore.save(this, request)
        startActivity(
            Intent(this, InitialLaunchFigureOuter::class.java).apply {
                addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP)
            },
        )
        finish()
    }
}
