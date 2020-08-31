package app.lockbook

import android.app.Application
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleObserver
import androidx.lifecycle.OnLifecycleEvent
import androidx.lifecycle.ProcessLifecycleOwner
import androidx.preference.PreferenceManager
import androidx.work.Constraints
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import app.lockbook.loggedin.listfiles.FileModel
import app.lockbook.utils.CoreModel
import app.lockbook.utils.LOG_FILE_NAME
import app.lockbook.utils.SharedPreferences.BACKGROUND_SYNC_ENABLED_KEY
import app.lockbook.utils.SharedPreferences.BACKGROUND_SYNC_PERIOD_KEY
import app.lockbook.utils.SharedPreferences.LOGGED_IN_KEY
import app.lockbook.utils.WorkManagerTags.PERIODIC_SYNC_TAG
import com.github.michaelbull.result.Err
import java.io.File
import java.util.concurrent.TimeUnit

class App : Application() {
    override fun onCreate() {
        super.onCreate()
        loadLockbookCore()
        ProcessLifecycleOwner.get().lifecycle
            .addObserver(ForegroundBackgroundObserver())
        instance = this
    }

    companion object {
        lateinit var instance: App
            private set
    }

    private fun loadLockbookCore() {
        System.loadLibrary("lockbook_core")
        val initLoggerResult = CoreModel.setUpInitLogger(filesDir.absolutePath)
        if (initLoggerResult is Err) {
            val logFile = File("$filesDir/$LOG_FILE_NAME")
            logFile.createNewFile()
            logFile.writeText("Cannot startup init_logger: ${initLoggerResult.error}")
        }
    }
}

class ForegroundBackgroundObserver : LifecycleObserver {

    @OnLifecycleEvent(Lifecycle.Event.ON_START)
    fun onMoveToForeground() {
        if (PreferenceManager.getDefaultSharedPreferences(App.instance)
            .getBoolean(LOGGED_IN_KEY, false)
        ) {
            WorkManager.getInstance(App.instance)
                .cancelAllWorkByTag(PERIODIC_SYNC_TAG)
        }
    }

    @OnLifecycleEvent(Lifecycle.Event.ON_STOP)
    fun onMoveToBackground() {
        if (PreferenceManager.getDefaultSharedPreferences(App.instance)
            .getBoolean(LOGGED_IN_KEY, false) && PreferenceManager.getDefaultSharedPreferences(
                App.instance
            )
                .getBoolean(BACKGROUND_SYNC_ENABLED_KEY, true)
        ) {
            val work = PeriodicWorkRequestBuilder<FileModel.SyncWork>(
                PreferenceManager.getDefaultSharedPreferences(App.instance).getString(BACKGROUND_SYNC_PERIOD_KEY, "30")?.toLongOrNull() ?: 30,
                TimeUnit.MINUTES
            )
                .setConstraints(Constraints.NONE)
                .addTag(PERIODIC_SYNC_TAG)
                .build()

            WorkManager.getInstance(App.instance)
                .enqueueUniquePeriodicWork(
                    PERIODIC_SYNC_TAG,
                    ExistingPeriodicWorkPolicy.REPLACE,
                    work
                )
        }
    }
}
