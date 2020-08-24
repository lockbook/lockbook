package app.lockbook

import app.lockbook.core.loadLockbookCore
import app.lockbook.utils.Config
import app.lockbook.utils.CoreModel
import app.lockbook.utils.CreateFileError
import app.lockbook.utils.FileType
import com.beust.klaxon.Klaxon
import org.junit.Before
import org.junit.Test

class CreateInsertFileTest {

    private val coreModel = CoreModel(Config(path))

    @Before
    fun loadLib() {
        loadLockbookCore()
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
    }

    @Test
    fun createInsertDocument() {
        val document = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Document)).component1()!!
        coreModel.insertFile(document).component1()!!
    }

    @Test
    fun createInsertFolder() {
        val folder = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Folder)).component1()!!
        coreModel.insertFile(folder).component1()!!
    }

    @Test
    fun createFileContainsSlash() {
        val document = coreModel.createFile("/", Klaxon().toJsonString(FileType.Document)).component2()!!
        val folder = coreModel.createFile("/", Klaxon().toJsonString(FileType.Folder)).component2()!!
        require(document is CreateFileError.FileNameContainsSlash)
        require(folder is CreateFileError.FileNameContainsSlash)
    }
}