package app.lockbook

import app.lockbook.core.migrateDB
import app.lockbook.model.CoreModel
import app.lockbook.util.Config
import app.lockbook.util.IntermCoreResult
import app.lockbook.util.MigrationError
import kotlinx.serialization.decodeFromString
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
        CoreModel.getDBState(config).unwrapOk()

        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        CoreModel.migrateDB(config).unwrapOk()
    }

    @Test
    fun getDBStateUnexpectedError() {
        CoreModel.migrateDBParser.decodeFromString<IntermCoreResult<Unit, MigrationError>>(
            migrateDB("")
        ).unwrapUnexpected()
    }
}
