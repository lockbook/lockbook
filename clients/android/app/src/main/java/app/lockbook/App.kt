package app.lockbook

import android.app.Application
import android.content.Context
import androidx.appcompat.app.AppCompatDelegate
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ProcessLifecycleOwner
import androidx.preference.PreferenceManager
import androidx.work.*
import app.lockbook.App.Companion.PERIODIC_SYNC_TAG
import app.lockbook.billing.BillingClientLifecycle
import app.lockbook.util.*
import app.lockbook.workspace.Workspace
import com.google.android.material.color.DynamicColors
import net.lockbook.Lb
import net.lockbook.LbError
import timber.log.Timber
import java.util.concurrent.TimeUnit

class App : Application() {
    val billingClientLifecycle: BillingClientLifecycle
        get() = BillingClientLifecycle.getInstance(this)

    var isInImportSync = false

    init {
        instance = this
    }

    override fun onCreate() {
        super.onCreate()
        DynamicColors.applyToActivitiesIfAvailable(this)
        Timber.plant(Timber.DebugTree())
        Workspace.init()
        Lb.init(filesDir.absolutePath)

        ProcessLifecycleOwner.get().lifecycle
            .addObserver(ForegroundBackgroundObserver(this))

        AppCompatDelegate.setDefaultNightMode(AppCompatDelegate.MODE_NIGHT_FOLLOW_SYSTEM)
    }

    companion object {
        const val PERIODIC_SYNC_TAG = "periodic_sync"
        var instance: App? = null

        fun applicationContext(): Context {
            return instance!!.applicationContext
        }
    }
}

class ForegroundBackgroundObserver(val context: Context) : DefaultLifecycleObserver {
    override fun onStart(owner: LifecycleOwner) {
        doIfLoggedIn {
            WorkManager.getInstance(context)
                .cancelAllWorkByTag(PERIODIC_SYNC_TAG)
        }
    }

    override fun onStop(owner: LifecycleOwner) {
        doIfLoggedIn {
            val work = PeriodicWorkRequestBuilder<SyncWork>(
                PreferenceManager.getDefaultSharedPreferences(context)
                    .getInt(getString(context.resources, R.string.background_sync_period_key), 30)
                    .toLong(),
                TimeUnit.MINUTES
            )
                .setConstraints(Constraints.NONE)
                .addTag(PERIODIC_SYNC_TAG)
                .build()

            WorkManager.getInstance(context)
                .enqueueUniquePeriodicWork(
                    PERIODIC_SYNC_TAG,
                    ExistingPeriodicWorkPolicy.REPLACE,
                    work
                )
        }
    }

    private fun doIfLoggedIn(onSuccess: () -> Unit) {
        if (!(context.applicationContext as App).isInImportSync) {
            try {
                Lb.getAccount()
                onSuccess()
            } catch (err: LbError) {
                Timber.e("Error: ${err.msg}")
            }
        }
    }
}

class SyncWork(appContext: Context, workerParams: WorkerParameters) :
    Worker(appContext, workerParams) {
    override fun doWork(): Result = try {
//        Lb.sync(null)

        Result.success()
    } catch (err: LbError) {
        Timber.e(err.msg)

        Result.failure()
    }
}
