package app.lockbook.util

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job

class ThreadHelper(private val job: Job, private val uiScope: CoroutineScope = CoroutineScope(Dispatchers.Main + job)) {
    fun launch(work: () -> Unit) {

    }
}