package app.lockbook

import app.lockbook.util.CoreError
import app.lockbook.util.UiCoreError
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.getOrElse
import com.github.michaelbull.result.unwrapError
import java.util.*

// You have to build the jni from core first to be able to run the tests.
// Next you have to add a vm option that helps java find the library:
// -ea -Djava.library.path="<path to lockbook>/lockbook/clients/android/core/src/main/jniLibs/desktop/"

fun generateAlphaString(): String =
    (1..20).map { (('A'..'Z') + ('a'..'z')).random() }.joinToString("")

fun generateId(): String = UUID.randomUUID().toString()

fun generateFakeRandomPath() = "/tmp/${System.currentTimeMillis()}"

fun createRandomPath(): String {
    val path = "/tmp/${generateAlphaString()}"
    Runtime.getRuntime().exec("mkdir $path")
    return path
}

fun <E> Result<*, CoreError<E>>?.unwrapErrorType(enumType: UiCoreError): UiCoreError
where E : Enum<E>, E : UiCoreError {
    val error = this?.unwrapError()
    require(error is CoreError.UiError && error.content == enumType) {
        "${Thread.currentThread().stackTrace[1]}: ${if (error == null) "null" else error::class.qualifiedName} is not of type ${enumType::class.qualifiedName}"
    }

    return error.content
}

fun <O, E> Result<O, CoreError<E>>?.unwrapOk(): O
        where E : Enum<E>, E : UiCoreError {
    return this!!.getOrElse { error ->
        val errorName = if (error is CoreError.UiError) {
            error.content
        } else if (error is CoreError.UiError) {
            error.content.name
        } else {
            "a 3rd unknown (and impossible) core error type"
        }

        throw Throwable("${Thread.currentThread().stackTrace[1]}: Tried unwrap on error type $errorName")
    }
}
