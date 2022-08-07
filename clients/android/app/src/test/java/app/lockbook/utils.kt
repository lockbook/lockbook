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

fun <E, A> Result<*, CoreError<E>>?.unwrapErrorType(enumType: A): UiCoreError
where E : Enum<E>, E : UiCoreError, A : Enum<E>, A : UiCoreError {
    val error = this?.unwrapError()
    require(error != null) {
        "${Thread.currentThread().stackTrace[1]}: null is not of type ${enumType.name}"
    }

    require(error is CoreError.UiError) {
        "${Thread.currentThread().stackTrace[1]}: ${error::class.qualifiedName} is not of type ${enumType.name}" +
            "\nmsg: ${(error as CoreError.Unexpected).content}"
    }

    require(error.content == enumType) {
        "${Thread.currentThread().stackTrace[1]}: ${error.content.name} is not of type ${enumType.name}"
    }

    return error.content
}

fun <O, E> Result<O, CoreError<E>>?.unwrapOk(): O
        where E : Enum<E>, E : UiCoreError {
    return this!!.getOrElse { error ->
        val errorName = when (error) {
            is CoreError.UiError -> {
                error.content.name
            }
            is CoreError.Unexpected -> {
                error.content
            }
            else -> {
                "a 3rd unknown (and impossible) core error type"
            }
        }

        throw Throwable("${Thread.currentThread().stackTrace[1]}: Tried unwrap on error: $errorName")
    }
}
