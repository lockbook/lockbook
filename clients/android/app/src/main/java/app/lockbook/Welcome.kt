package app.lockbook

import android.content.Intent
import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity
import kotlinx.android.synthetic.main.activity_main.*

class Welcome : AppCompatActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        new_lockbook.setOnClickListener {
            startActivity(Intent(applicationContext, NewAccount::class.java))
        }

        import_lockbook.setOnClickListener {
            startActivity(Intent(applicationContext, ImportAccount::class.java))
        }
    }
}
