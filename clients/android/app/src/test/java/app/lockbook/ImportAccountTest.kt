package app.lockbook

import app.lockbook.core.importAccount
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.unwrap
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
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        val exportAccountString = CoreModel.exportAccount(config).unwrap()

        config = Config(createRandomPath())

        CoreModel.importAccount(config, exportAccountString).unwrap()
    }

    @Test
    fun importAccountStringCorrupted() {
        CoreModel.importAccount(config, "!@#$%^&*()")
            .unwrapErrorType<ImportError.AccountStringCorrupted>()
    }

    @Test
    fun importAccountUnexpectedError() {
        Klaxon().converter(importAccountConverter)
            .parse<Result<Unit, ImportError>>(importAccount("", ""))
            .unwrapErrorType<ImportError.Unexpected>()
    }
}
