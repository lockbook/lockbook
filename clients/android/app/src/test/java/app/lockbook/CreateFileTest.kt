package app.lockbook

import app.lockbook.core.loadLockbookCore
import app.lockbook.utils.Config
import app.lockbook.utils.CoreModel
import app.lockbook.utils.CreateFileError
import app.lockbook.utils.FileType
import com.beust.klaxon.Klaxon
import org.junit.After
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class CreateFileTest {
    private val coreModel = CoreModel(Config(path))

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            loadLockbookCore()
            Runtime.getRuntime().exec("mkdir $path")
        }
    }

    @After
    fun resetDirectory() {
        Runtime.getRuntime().exec("rm -rf $path/*")
    }

    @Test
    fun createFileOk() {
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Document)).component1()!!
    }

    @Test
    fun createFileContainsSlash() {
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val document = coreModel.createFile("/", Klaxon().toJsonString(FileType.Document)).component2()!!
        val folder = coreModel.createFile("/", Klaxon().toJsonString(FileType.Folder)).component2()!!
        require(document is CreateFileError.FileNameContainsSlash)
        require(folder is CreateFileError.FileNameContainsSlash)
    }

    @Test
    fun createFileNotAvailable() {
        val fileName = generateAlphaString()
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val document = coreModel.createFile(fileName, Klaxon().toJsonString(FileType.Document)).component1()!!
        coreModel.insertFile(document).component1()!!
        val folder = coreModel.createFile(fileName, Klaxon().toJsonString(FileType.Folder)).component2()!!
        require(folder is CreateFileError.FileNameNotAvailable)
    }

    @Test
    fun createFileNoAccount() {
        val createFileError = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Document)).component2()!!
        require(createFileError is CreateFileError.NoAccount)
    }
}