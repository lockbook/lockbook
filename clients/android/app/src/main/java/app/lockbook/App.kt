package app.lockbook

import android.app.Application
import android.content.Context
import android.content.pm.ApplicationInfo
import android.os.StrictMode
import android.os.StrictMode.ThreadPolicy
import android.os.StrictMode.VmPolicy
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
import java.io.PrintWriter
import java.io.StringWriter
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

        val isDebugBuild = 0 != applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE
        if (isDebugBuild) {
            StrictMode.setThreadPolicy(
                ThreadPolicy.Builder()
                    .detectDiskReads()
                    .detectDiskWrites()
                    .detectAll()
                    .penaltyLog()
                    .build()
            )

            StrictMode.setVmPolicy(
                VmPolicy.Builder()
                    .detectLeakedSqlLiteObjects()
                    .detectLeakedClosableObjects()
                    .penaltyLog()
                    .build()
            )
        }

        val defaultHandler = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler(
            GlobalExceptionHandler(this, defaultHandler)
        )
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
        Lb.sync(null)

        Result.success()
    } catch (err: LbError) {
        Timber.e(err.msg)

        Result.failure()
    }
}

class GlobalExceptionHandler(
    private val context: Context,
    private val defaultHandler: Thread.UncaughtExceptionHandler?
) : Thread.UncaughtExceptionHandler {

    override fun uncaughtException(thread: Thread, exception: Throwable) {
        try {
            println("custom uncaught handler")
            val stackTrace = getStackTraceString(exception)
            Lb.writePanicToFile(exception.message, stackTrace)
        } catch (e: Exception) {
            println("custom uncaught handler failed")

            println(e)
        } finally {
            defaultHandler?.uncaughtException(thread, exception)
        }
    }

    private fun getStackTraceString(exception: Throwable): String {
        val sw = StringWriter()

        val pw = PrintWriter(sw)
        exception.printStackTrace(pw)
        return sw.toString()
    }
}
