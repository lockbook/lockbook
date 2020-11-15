package app.lockbook

import app.lockbook.core.importAccount
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class ImportAccountTest {
    var config = Config(createRandomPath())

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            System.loadLibrary("lockbook_core")
        }
    }

    @After
    fun createDirectory() {
        config = Config(createRandomPath())
    }

    @Test
    fun importAccountOk() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val exportAccountString = assertTypeReturn<String>(
            CoreModel.exportAccount(config).component1()
        )

        config = Config(createRandomPath())

        assertType<Unit>(
            CoreModel.importAccount(config, exportAccountString).component1()
        )
    }

    @Test
    fun importAccountStringCorrupted() {
        assertType<ImportError.AccountStringCorrupted>(
            CoreModel.importAccount(config, "!@#$%^&*()").component2()
        )
    }

    @Test
    fun importAccountUnexpectedError() {
        val importResult: Result<Unit, ImportError>? =
            Klaxon().converter(importAccountConverter)
                .parse(importAccount("", ""))

        assertType<ImportError.Unexpected>(
            importResult?.component2()
        )
    }
}
