package app.lockbook

import android.content.Intent
import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity
import androidx.databinding.DataBindingUtil
import app.lockbook.databinding.ActivityMainBinding
import app.lockbook.login.ImportAccountActivity
import app.lockbook.login.NewAccountActivity

class WelcomeActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val binding: ActivityMainBinding = DataBindingUtil.setContentView(this, R.layout.activity_main)
        binding.welcomeActivity = this
    }

    fun launchNewAccount() {
        startActivity(Intent(applicationContext, NewAccountActivity::class.java))
    }

    fun launchImportAccount() {
        startActivity(Intent(applicationContext, ImportAccountActivity::class.java))
    }

}
