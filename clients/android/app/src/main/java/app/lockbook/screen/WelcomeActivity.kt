package app.lockbook.screen

import android.content.Intent
import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.databinding.ActivityMainBinding

class WelcomeActivity : AppCompatActivity() {
    private var _binding: ActivityMainBinding? = null
    // This property is only valid between onCreateView and
    // onDestroyView.
    private val binding get() = _binding!!

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        _binding = ActivityMainBinding.inflate(layoutInflater)
        setContentView(binding.root)

        binding.welcomeNewLockbook.setOnClickListener {
            launchNewAccount()
        }

        binding.welcomeImportLockbook.setOnClickListener {
            launchImportAccount()
        }
    }

    private fun launchNewAccount() {
        startActivity(Intent(applicationContext, NewAccountActivity::class.java))
    }

    private fun launchImportAccount() {
        startActivity(Intent(applicationContext, ImportAccountActivity::class.java))
    }
}
