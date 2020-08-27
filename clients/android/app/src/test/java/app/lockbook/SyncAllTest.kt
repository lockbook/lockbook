package app.lockbook

import app.lockbook.core.loadLockbookCore
import app.lockbook.core.syncAll
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class SyncAllTest {

    private val coreModel = CoreModel(Config(path))
    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            loadLockbookCore()
            Runtime.getRuntime().exec("rm -rf $path")
        }
    }

    @Before
    fun createDirectory() {
        Runtime.getRuntime().exec("mkdir $path")
    }

    @After
    fun resetDirectory() {
        Runtime.getRuntime().exec("rm -rf $path")
    }

    @Test
    fun syncAllOk() {
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val document = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Document)).component1()!!
        coreModel.insertFile(document).component1()!!
        val folder = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Folder)).component1()!!
        coreModel.insertFile(folder).component1()!!
        CoreModel.syncAllFiles(Config(path)).component1()!!
    }

    @Test
    fun syncAllNoAccount() {
        val syncAllError = CoreModel.syncAllFiles(Config(path)).component2()!!
        require(syncAllError is SyncAllError.NoAccount)
    }

    @Test
    fun syncAllUnexpectedError() {
        val syncResult: Result<Unit, SyncAllError>? =
            Klaxon().converter(syncAllConverter).parse(syncAll(Klaxon().toJsonString("")))
        val syncError = syncResult!!.component2()!!
        require(syncError is SyncAllError.UnexpectedError)
    }
}
