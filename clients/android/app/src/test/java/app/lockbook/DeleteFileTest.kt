package app.lockbook

import app.lockbook.core.deleteFile
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.*

@Ignore("Delete endpoint doesn't work yet")
class DeleteFileTest {
    var path = createRandomPath()

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            System.loadLibrary("lockbook_core")
        }
    }

    @After
    fun createDirectory() {
        path = createRandomPath()
    }

    @Test
    fun deleteFileOk() {
        val coreModel = CoreModel(Config(path))
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val document = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Document)).component1()!!
        coreModel.insertFile(document).component1()!!
        val folder = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Folder)).component1()!!
        coreModel.insertFile(folder).component1()!!
        coreModel.deleteFile(document.id).component1()!!
        coreModel.deleteFile(folder.id).component1()!!
    }

    @Test
    fun deleteFileNoFileWithThatId() {
        val coreModel = CoreModel(Config(path))
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val deleteFileError = coreModel.deleteFile(generateId()).component2()!!
        require(deleteFileError is DeleteFileError.NoFileWithThatId) {
            "${Klaxon().toJsonString(deleteFileError)} != ${DeleteFileError.NoFileWithThatId::class.qualifiedName}"
        }
    }

    @Test
    fun deleteFileUnexpectedError() {
        val deleteFile: Result<Unit, DeleteFileError>? =
            Klaxon().converter(deleteFileConverter).parse(deleteFile("", ""))
        val deleteFileError = deleteFile!!.component2()!!
        require(deleteFileError is DeleteFileError.UnexpectedError) {
            "${Klaxon().toJsonString(deleteFileError)} != ${DeleteFileError.UnexpectedError::class.qualifiedName}"
        }
    }
}
