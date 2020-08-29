package app.lockbook

import app.lockbook.core.importAccount
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class ImportAccountTest {
    var path = createRandomPath()

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            System.loadLibrary("lockbook_core")
        }
    }

    @After
    fun createDirectory() {
        path = createRandomPath()
    }

    @Test
    fun importAccountOk() {
        CoreModel.generateAccount(Config(path), generateAlphaString()).component1()!!
        val exportAccountString = CoreModel.exportAccount(Config(path)).component1()!!
        path = createRandomPath()

        CoreModel.importAccount(Config(path), exportAccountString).component1()!!
    }

    @Test
    fun importAccountStringCorrupted() {
        val firstImportAccountError = CoreModel.importAccount(
            Config(path),
            "!@#$%^&*()"
        ).component2()!!
        require(firstImportAccountError is ImportError.AccountStringCorrupted) {
            "${Klaxon().toJsonString(firstImportAccountError)} != ${ImportError.AccountStringCorrupted::class.qualifiedName}"
        }
    }

    @Test
    fun importAccountUnexpectedError() {
        val importResult: Result<Unit, ImportError>? =
            Klaxon().converter(importAccountConverter)
                .parse(importAccount("", ""))
        val importError = importResult!!.component2()!!
        require(importError is ImportError.UnexpectedError) {
            "${Klaxon().toJsonString(importError)} != ${ImportError.UnexpectedError::class.qualifiedName}"
        }
    }
}
