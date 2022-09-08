package app.lockbook.screen

import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.R
import app.lockbook.util.maybeGetCreateLinkFragment
import com.google.android.material.appbar.MaterialToolbar

class SharedFilesActivity: AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_shared_files)
    }

    override fun onBackPressed() {
        maybeGetCreateLinkFragment()?.onBackPressed()
    }
}