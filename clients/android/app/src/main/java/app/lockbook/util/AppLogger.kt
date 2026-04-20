package app.lockbook.util

import android.util.Log
import timber.log.Timber

object AppLogger {

    // Top-level debug flag — not class-level, scoped to this object
    private val DEBUG = true

    fun getLogger(tag: String): Logger = if (DEBUG) DebugLogger(tag) else NoOpLogger

    init{
        if (DEBUG) Timber.plant(Timber.DebugTree())
    }
    interface Logger {
        fun d(msg: String)
        fun i(msg: String)
        fun w(msg: String, throwable: Throwable? = null)
        fun e(msg: String, throwable: Throwable? = null)
    }

    private class DebugLogger(private val tag: String) : Logger {
        override fun d(msg: String) = Timber.d("%s: %s", tag, msg)
        override fun i(msg: String) = Timber.i("%s: %s", tag, msg)
        override fun w(msg: String, throwable: Throwable?) = Timber.w(throwable, "%s: %s", tag, msg)
        override fun e(msg: String, throwable: Throwable?) = Timber.e(throwable, "%s: %s", tag, msg)
    }

    private object NoOpLogger : Logger {
        override fun d(msg: String) = Unit
        override fun i(msg: String) = Unit
        override fun w(msg: String, throwable: Throwable?) = Unit
        override fun e(msg: String, throwable: Throwable?) = Unit
    }
}