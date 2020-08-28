package app.lockbook

import app.lockbook.core.importAccount
import app.lockbook.core.loadLockbookCore
import app.lockbook.utils.Config
import app.lockbook.utils.CoreModel
import app.lockbook.utils.ImportError
import app.lockbook.utils.importAccountConverter
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
            loadLockbookCore()
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
        require(firstImportAccountError is ImportError.AccountStringCorrupted)
    }

    @Test
    fun importAccountUnexpectedError() {
        val importResult: Result<Unit, ImportError>? =
            Klaxon().converter(importAccountConverter)
                .parse(importAccount("", ""))
        val importError = importResult!!.component2()!!
        require(importError is ImportError.UnexpectedError)
    }
}
