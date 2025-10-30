package app.lockbook.services

import android.app.Application
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Intent
import android.os.Build
import androidx.annotation.RequiresApi
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LifecycleService
import androidx.lifecycle.viewModelScope
import app.lockbook.R
import app.lockbook.model.NotifySyncDone
import app.lockbook.model.SyncRepository
import app.lockbook.model.SyncStepInfo
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import net.lockbook.Lb
import net.lockbook.LbError

const val ANDROID_CHANNEL_ID = "lockbook_initial_sync"
const val ACCOUNT_IMPORT_KEY = "account_import_key"
const val SYNC_PROGRESS_NOTIFICATION_ID = 1
class InitialSync : LifecycleService() {

    private val scope = CoroutineScope(Dispatchers.IO + Job())

    private lateinit var notificationManager: NotificationManager

    private val syncRepository = SyncRepository.getInstance()

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
        startForeground(SYNC_PROGRESS_NOTIFICATION_ID, notification)

        scope.launch {
            val account =  Lb.getAccount()
            println("starting to sync " + account.username + " data" )
            syncRepository.trySync()
        }

        syncRepository.notifySyncStepInfo.observe(
            this
        ) { stepInfo ->
            println("new notification")
            val notification = createNotificationWithProgress(stepInfo.progress, stepInfo.total)
            notificationManager.notify(SYNC_PROGRESS_NOTIFICATION_ID, notification)
        }

        return super.onStartCommand(intent, flags, startId)
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
            .setContentTitle("Importing your Lockbook")
            .setContentText("Syncing $progress out of $max files")
            .setSmallIcon(R.drawable.large_foreground) // REQUIRED: Add a small icon
            .setProgress(max, progress, false)
            .setOngoing(true)
            .build()
    }
}

class ImportAccountViewModel(application: Application) : AndroidViewModel(application) {
    val syncRepository = SyncRepository.getInstance()

    var isErrorVisible = false

    init {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                syncRepository.trySync()
            } catch (err: LbError) {
                isErrorVisible = true
            }
        }
    }
}
