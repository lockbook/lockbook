package app.lockbook

import android.content.Intent
import android.os.Bundle
import android.util.Log
import androidx.appcompat.app.AppCompatActivity
import androidx.databinding.DataBindingUtil
import app.lockbook.databinding.ActivityMainBinding

class Welcome : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val binding: ActivityMainBinding = DataBindingUtil.setContentView(this, R.layout.activity_main)
        binding.welcomeActivity = this
    }

    fun launchNewAccount() {
        startActivity(Intent(applicationContext, NewAccount::class.java))
        Log.i("info", "LAUNCHED NEW ACCOUNT")
    }

    fun launchImportAccount() {
        startActivity(Intent(applicationContext, ImportAccount::class.java))
        Log.i("info", "LAUNCHED IMPORT ACCOUNT")

    }

}
