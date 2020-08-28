package app.lockbook

import app.lockbook.core.createFile
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class CreateFileTest {
    var path = createRandomPath()

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            System.loadLibrary("lockbook_core")
        }
    }

    @Before
    fun createDirectory() {
        path = createRandomPath()
    }

    @Test
    fun createFileOk() {
        val coreModel = CoreModel(Config(path))
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Document)).component1()!!
    }

    @Test
    fun createFileContainsSlash() {
        val coreModel = CoreModel(Config(path))
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
        val coreModel = CoreModel(Config(path))
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
        val coreModel = CoreModel(Config(path))
        CoreModel.generateAccount(Config(path), generateAlphaString()).component1()!!
        coreModel.setParentToRoot().component1()!!
        Runtime.getRuntime().exec("rm -rf $path")
        Runtime.getRuntime().exec("mkdir $path")

        val createFileError = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Document)).component2()!!
        require(createFileError is CreateFileError.NoAccount)
    }

    @Test
    fun createFileUnexpectedError() {
        val createFileResult: Result<FileMetadata, CreateFileError>? =
            Klaxon().converter(createFileConverter)
                .parse(createFile("", "", "", ""))
        val createFileError = createFileResult!!.component2()!!
        require(createFileError is CreateFileError.UnexpectedError)
    }
}
