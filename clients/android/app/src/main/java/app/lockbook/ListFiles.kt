package app.lockbook

import android.os.Bundle
import android.os.Handler
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import kotlin.system.exitProcess

class ListFiles : AppCompatActivity() {

    private var exit = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.list_files)
    }

    override fun onBackPressed() {
        if(exit) {
            finish()
            exitProcess(1) // may be changed
        } else {
            Toast.makeText(this, "Press back again to exit.", Toast.LENGTH_SHORT).show()
            exit = true
            Handler().postDelayed({
                exit = false
            }, 3000)
        }
    }
}
