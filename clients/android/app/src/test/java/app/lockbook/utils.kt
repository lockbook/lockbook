package app.lockbook

import java.util.*

// You have to build the jni from core first to be able to run the tests.
// Next you have to add a vm option that helps java find the library:
// -ea -Djava.library.path="<path to lockbook>/lockbook/clients/android/core/src/main/jniLibs/desktop/"

fun generateAlphaString(): String =
    (1..20).map { (('A'..'Z') + ('a'..'z')).random() }.joinToString("")

fun generateId(): String = UUID.randomUUID().toString()

fun createRandomPath(): String {
    val path = "/tmp/${generateAlphaString()}"
    Runtime.getRuntime().exec("mkdir $path")
    return path
}

inline fun <reified T> assertType(methodName: String, comparableValue: Any?) {
    require(comparableValue is T) {
        "$methodName: ${if (comparableValue == null) "null" else comparableValue::class.qualifiedName} is not of type ${T::class.qualifiedName}"
    }
}

inline fun <reified T> assertTypeReturn(methodName: String, comparableValue: Any?): T {
    require(comparableValue is T) {
        "$methodName: ${if (comparableValue == null) "null" else comparableValue::class.qualifiedName} is not of type ${T::class.qualifiedName}"
    }

    return comparableValue
}
