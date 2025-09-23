package app.lockbook.services

import android.Manifest
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.IBinder
import androidx.core.app.ActivityCompat
import app.lockbook.R
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import androidx.core.content.PermissionChecker
import app.lockbook.screen.ShareReceiverActivity
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.cancel
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.launch
import net.lockbook.Lb
import net.lockbook.LbError


const val ANDROID_CHANNEL_ID = "lockbook_initial_sync"
const val ACCOUNT_IMPORT_KEY = "account_import_key"

class InitialSync : Service() {


    private val scope = CoroutineScope(Dispatchers.IO + Job())

    private lateinit var notificationManager: NotificationManager

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
        notificationManager = getSystemService(NotificationManager::class.java)

    }
    override fun onDestroy() {
        super.onDestroy()
        scope.cancel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {

        val notification = createNotificationWithProgress(0)
        startForeground(1, notification)

        intent?.extras?.getString(ACCOUNT_IMPORT_KEY)?.let { account ->
            println("account key is $account")
            scope.launch {
                val progress = 50
                val notification = createNotificationWithProgress(progress)
                notificationManager.notify(progress, notification)
                // todo i gotta handle the first sync here too because import
                // does actually import the data from network.
                // there's a bunch of logic storred in importaccountviewmodel that needs to be
                // transfered here
                Lb.importAccount(account)
            }
        }

        return START_STICKY
    }

    private fun createNotificationChannel() {
        val channel = NotificationChannel(
            ANDROID_CHANNEL_ID,
            "Initial Sync Progress",
            NotificationManager.IMPORTANCE_LOW
        ).apply {
            description = "Lockbook background service"
        }

        val notificationManager = getSystemService(NotificationManager::class.java)
        notificationManager.createNotificationChannel(channel)
    }

    private fun createNotificationWithProgress(progress: Int, max: Int = 100): Notification {
        return Notification.Builder(this, ANDROID_CHANNEL_ID)
            .setContentTitle("Importing Lockbook Account")
            .setContentText("Syncing $progress out of $max files")
            .setSmallIcon(R.drawable.large_foreground) // REQUIRED: Add a small icon
            .setProgress(100, 50, false)
            .setOngoing(true)
            .build()
    }
}