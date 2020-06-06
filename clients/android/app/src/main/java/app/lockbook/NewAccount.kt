package app.lockbook

import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.core.createAccount
import kotlinx.android.synthetic.main.new_account.*

class NewAccount : AppCompatActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.new_account)

        create_lockbook.setOnClickListener {
            println(createAccount(filesDir.absolutePath, username.text.toString()))
        }
    }
}
