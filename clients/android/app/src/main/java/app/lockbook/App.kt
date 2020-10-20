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
import app.lockbook.utils.SharedPreferences.BACKGROUND_SYNC_ENABLED_KEY
import app.lockbook.utils.SharedPreferences.BACKGROUND_SYNC_PERIOD_KEY
import app.lockbook.utils.SharedPreferences.IS_THIS_AN_IMPORT_KEY
import app.lockbook.utils.SharedPreferences.LOGGED_IN_KEY
import app.lockbook.utils.WorkManagerTags.PERIODIC_SYNC_TAG
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
        CoreModel.setUpInitLogger(filesDir.absolutePath)
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
            .getBoolean(LOGGED_IN_KEY, false) && PreferenceManager.getDefaultSharedPreferences(App.instance)
                .getBoolean(
                        BACKGROUND_SYNC_ENABLED_KEY,
                        true
                    ) && !PreferenceManager.getDefaultSharedPreferences(App.instance)
                .getBoolean(IS_THIS_AN_IMPORT_KEY, false)
        ) {
            val work = PeriodicWorkRequestBuilder<FileModel.SyncWork>(
                PreferenceManager.getDefaultSharedPreferences(App.instance)
                    .getInt(BACKGROUND_SYNC_PERIOD_KEY, 30).toLong(),
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
