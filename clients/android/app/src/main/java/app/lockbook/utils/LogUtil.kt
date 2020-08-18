package app.lockbook.utils

import java.io.BufferedReader
import java.io.InputStreamReader

class LogUtil {
    companion object {
        fun readLogs(): StringBuilder {
            val processId = android.os.Process.myPid().toString()
            val logBuilder = StringBuilder()
            val bufferedReader = BufferedReader(
                InputStreamReader(
                    Runtime.getRuntime().exec("logcat -d").inputStream
                )
            )
            bufferedReader.forEachLine { line -> if (line.contains(processId)) logBuilder.append("$line\n") }

            return logBuilder
        }
    }
}
