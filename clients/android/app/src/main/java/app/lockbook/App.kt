
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
import app.lockbook.App.Companion.config
import app.lockbook.model.CoreModel
import app.lockbook.util.Config
import app.lockbook.util.SharedPreferences.BACKGROUND_SYNC_PERIOD_KEY
import app.lockbook.util.State
import app.lockbook.util.SyncAllError
import app.lockbook.util.exhaustive
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import timber.log.Timber
import java.util.concurrent.TimeUnit

class App : Application() {
    override fun onCreate() {
        super.onCreate()
        loadLockbookCore()
        ProcessLifecycleOwner.get().lifecycle
            .addObserver(ForegroundBackgroundObserver())
        instance = this
        config = Config(this.filesDir.absolutePath)

        AppCompatDelegate.setDefaultNightMode(AppCompatDelegate.MODE_NIGHT_FOLLOW_SYSTEM)
    }

    companion object {
        lateinit var instance: App
            private set

        lateinit var config: Config
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
        doIfLoggedIn {
            WorkManager.getInstance(App.instance)
                .cancelAllWorkByTag(PERIODIC_SYNC_TAG)
        }
    }

    @OnLifecycleEvent(Lifecycle.Event.ON_STOP)
    fun onMoveToBackground() {
        doIfLoggedIn {
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

    private fun doIfLoggedIn(onSuccess: () -> Unit) {
        when(val getDbStateResult = CoreModel.getDBState(config)) {
            is Ok -> if (getDbStateResult.value == State.ReadyToUse) {
                onSuccess()
            }
            is Err -> Timber.e("Error: ${getDbStateResult.error.toLbError()}")
        }
    }
}

class SyncWork(appContext: Context, workerParams: WorkerParameters) :
    Worker(appContext, workerParams) {
    override fun doWork(): Result {
        val syncAllResult =
            CoreModel.sync(Config(applicationContext.filesDir.absolutePath), null)
        return if (syncAllResult is Err) {
            when (val error = syncAllResult.error) {
                is SyncAllError.NoAccount -> {
                    Timber.e("No account.")
                }
                is SyncAllError.CouldNotReachServer -> {
                    Timber.e("Could not reach server.")
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
