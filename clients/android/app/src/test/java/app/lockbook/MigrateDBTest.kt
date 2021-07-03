package app.lockbook

import app.lockbook.core.migrateDB
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.unwrap
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class MigrateDBTest {
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
    fun migrateDBOk() {
        CoreModel.getDBState(config).unwrap()

        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        CoreModel.migrateDB(config).unwrap()
    }

    @Test
    fun getDBStateUnexpectedError() {
        Klaxon().converter(migrateDBConverter).parse<Result<Unit, MigrationError>>(migrateDB(""))
            .unwrapErrorType<MigrationError.Unexpected>()
    }
}
