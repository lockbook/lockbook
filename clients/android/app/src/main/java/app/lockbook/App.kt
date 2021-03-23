package app.lockbook

import android.app.Application
import android.content.Context
import androidx.appcompat.app.AppCompatDelegate
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleObserver
import androidx.lifecycle.OnLifecycleEvent
import androidx.lifecycle.ProcessLifecycleOwner
import androidx.preference.PreferenceManager
import androidx.work.*
import app.lockbook.App.Companion.PERIODIC_SYNC_TAG
import app.lockbook.model.CoreModel
import app.lockbook.model.FileModel
import app.lockbook.util.Config
import app.lockbook.util.SharedPreferences.BACKGROUND_SYNC_ENABLED_KEY
import app.lockbook.util.SharedPreferences.BACKGROUND_SYNC_PERIOD_KEY
import app.lockbook.util.SharedPreferences.IS_THIS_AN_IMPORT_KEY
import app.lockbook.util.SharedPreferences.LOGGED_IN_KEY
import app.lockbook.util.SyncAllError
import app.lockbook.util.exhaustive
import com.github.michaelbull.result.Err
import timber.log.Timber
import java.util.concurrent.TimeUnit

class App : Application() {
    override fun onCreate() {
        super.onCreate()
        loadLockbookCore()
        ProcessLifecycleOwner.get().lifecycle
            .addObserver(ForegroundBackgroundObserver())
        instance = this

        AppCompatDelegate.setDefaultNightMode(AppCompatDelegate.MODE_NIGHT_FOLLOW_SYSTEM)
    }

    companion object {
        lateinit var instance: App
            private set

        const val PERIODIC_SYNC_TAG = "periodic_sync"
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
            val work = PeriodicWorkRequestBuilder<SyncWork>(
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

class SyncWork(appContext: Context, workerParams: WorkerParameters) :
        Worker(appContext, workerParams) {
    override fun doWork(): Result {
        val syncAllResult =
                CoreModel.syncAll(Config(applicationContext.filesDir.absolutePath))
        return if (syncAllResult is Err) {
            when (val error = syncAllResult.error) {
                is SyncAllError.NoAccount -> {
                    Timber.e("No account.")
                }
                is SyncAllError.CouldNotReachServer -> {
                    Timber.e("Could not reach server.")
                }
                is SyncAllError.ExecuteWorkError -> {
                    Timber.e("Could not execute some work.")
                }
                is SyncAllError.ClientUpdateRequired -> {
                    Timber.e("Client update required.")
                }
                is SyncAllError.Unexpected -> {
                    Timber.e("Unable to sync all files: ${error.error}")
                }
            }.exhaustive
            Result.failure()
        } else {
            Result.success()
        }
    }
}

