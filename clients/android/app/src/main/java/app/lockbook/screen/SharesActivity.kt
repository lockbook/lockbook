package app.lockbook.screen

import android.annotation.SuppressLint
import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.R
import app.lockbook.util.maybeGetCreateLinkFragment

class SharesActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_pending_shares)
    }

    @SuppressLint("MissingSuperCall")
    override fun onBackPressed() {
        val maybeCreateLinkFragment = maybeGetCreateLinkFragment()

        if (maybeCreateLinkFragment != null) {
            maybeCreateLinkFragment.onBackPressed()
        } else {
            setResult(RESULT_OK)
            finish()
        }
    }
}
