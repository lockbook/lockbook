package app.lockbook

import app.lockbook.model.CoreModel
import app.lockbook.util.Config
import app.lockbook.util.FileType
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class SyncAllTest {

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            System.loadLibrary("lockbook_core")
        }
    }

    @Before
    fun initCore() {
        CoreModel.init(Config(false, createRandomPath()))
    }

    @Test
    fun syncAllOk() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        CoreModel.createFile(
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrapOk()

        CoreModel.createFile(
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrapOk()

        CoreModel.syncAll(null).unwrapOk()
    }
}
