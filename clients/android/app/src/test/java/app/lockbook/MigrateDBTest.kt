package app.lockbook

import app.lockbook.core.migrateDB
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
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
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        assertType<Unit>(
            CoreModel.migrateDB(config).component1()
        )
    }

    @Test
    fun getDBStateUnexpectedError() {
        assertType<MigrationError.Unexpected>(
            Klaxon().converter(migrateDBConverter).parse<Result<Unit, MigrationError>>(migrateDB(""))?.component2()
        )
    }
}
