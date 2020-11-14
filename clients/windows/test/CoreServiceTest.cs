using Core;
using lockbook;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using Newtonsoft.Json;
using Newtonsoft.Json.Converters;
using System;
using System.IO;

namespace test {
    [TestClass]
    public class CoreServiceTest {
        const string lockbookDir = "C:\\Temp\\.lockbook"; // todo: find a more suitable location

        public CoreService CoreService {
            get { return new CoreService(lockbookDir); }
        }

        public string RandomUsername() {
            return "testUsername" + Guid.NewGuid().ToString().Replace("-", "");
        }

        public TExpected CastOrDie<TExpected, TActual>(TActual actual, out TExpected expected) where TExpected : TActual {
            if (typeof(TExpected) == actual.GetType()) {
                expected = (TExpected)actual;
                return expected;
            }
            throw new InvalidCastException(
                string.Format(
                    "expected {0} but got {1}: {2}",
                    typeof(TExpected).FullName,
                    actual.GetType().FullName,
                    JsonConvert.SerializeObject(actual, new StringEnumConverter())));
        }

        [TestInitialize]
        public void DeleteAccount() {
            try {
                Directory.Delete(lockbookDir, true);
            } catch (DirectoryNotFoundException) { }
        }

        [TestMethod]
        public void GetDbState() {
            var getDbStateResult = CoreService.GetDbState().WaitResult();
            CastOrDie(getDbStateResult, out Core.GetDbState.Success _);
        }

        [TestMethod]
        public void MigrateDb() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            // needs to be done first
            var getDbStateResult = CoreService.GetDbState().WaitResult();
            CastOrDie(getDbStateResult, out Core.GetDbState.Success _);

            var migrateDbResult = CoreService.MigrateDb().WaitResult();
            CastOrDie(migrateDbResult, out Core.MigrateDb.Success _);
        }

        [TestMethod]
        public void AccountExistsFalse() {
            Assert.IsFalse(CoreService.AccountExists().WaitResult());
        }

        [TestMethod]
        public void AccountExistsTrue() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getAccountResult = CoreService.GetAccount().WaitResult();
            CastOrDie(getAccountResult, out Core.GetAccount.Success _);
            Assert.IsTrue(CoreService.AccountExists().WaitResult());
        }

        [TestMethod]
        public void CreateAccountSuccess() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);
        }

        [TestMethod]
        public void CreateAccountAccountExistsAlready() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var username2 = RandomUsername();
            var createAccountResult2 = CoreService.CreateAccount(username2).WaitResult();
            Assert.AreEqual(Core.CreateAccount.PossibleErrors.AccountExistsAlready,
                CastOrDie(createAccountResult2, out Core.CreateAccount.ExpectedError _).Error);
        }

        [TestMethod]
        public void CreateAccountUsernameTaken() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var syncResult = CoreService.SyncAll().WaitResult();
            CastOrDie(syncResult, out Core.SyncAll.Success _);

            Directory.Delete(lockbookDir, true);

            var createAccountResult2 = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(Core.CreateAccount.PossibleErrors.UsernameTaken,
                CastOrDie(createAccountResult2, out Core.CreateAccount.ExpectedError _).Error);
        }

        [TestMethod]
        public void CreateAccountInvalidUsername() {
            var username = "not! a! valid! username!";
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(Core.CreateAccount.PossibleErrors.InvalidUsername,
                CastOrDie(createAccountResult, out Core.CreateAccount.ExpectedError _).Error);
        }

        [TestMethod]
        public void GetAccount() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getAccountResult = CoreService.GetAccount().WaitResult();
            CastOrDie(getAccountResult, out Core.GetAccount.Success _);
        }

        [TestMethod]
        public void GetAccountNoAccount() {
            var getAccountResult = CoreService.GetAccount().WaitResult();
            Assert.AreEqual(Core.GetAccount.PossibleErrors.NoAccount,
               CastOrDie(getAccountResult, out Core.GetAccount.ExpectedError _).Error);
        }

        [TestMethod]
        public void ExportAccount() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var exportAccountResult = CoreService.ExportAccount().WaitResult();
            CastOrDie(exportAccountResult, out Core.ExportAccount.Success _);
        }

        [TestMethod]
        public void ExportAccountNoAccount() {
            var exportAccountResult = CoreService.ExportAccount().WaitResult();
            Assert.AreEqual(Core.ExportAccount.PossibleErrors.NoAccount,
               CastOrDie(exportAccountResult, out Core.ExportAccount.ExpectedError _).Error);
        }

        [TestMethod]
        public void ImportAccount() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var exportAccountResult = CoreService.ExportAccount().WaitResult();
            var accountString = CastOrDie(exportAccountResult, out Core.ExportAccount.Success _).accountString;

            Directory.Delete(lockbookDir, true);

            var importAccountResult = CoreService.ImportAccount(accountString).WaitResult();
            CastOrDie(importAccountResult, out Core.ImportAccount.Success _);
        }

        [TestMethod]
        public void ImportAccountAccountStringCorrupted() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var accountString = "#######!!@$@%";

            Directory.Delete(lockbookDir, true);

            var importAccountResult = CoreService.ImportAccount(accountString).WaitResult();
            Assert.AreEqual(Core.ImportAccount.PossibleErrors.AccountStringCorrupted,
                CastOrDie(importAccountResult, out Core.ImportAccount.ExpectedError _).Error);
        }

        [TestMethod]
        public void ImportAccountAccountExistsAlready() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var exportAccountResult = CoreService.ExportAccount().WaitResult();
            var accountString = CastOrDie(exportAccountResult, out Core.ExportAccount.Success _).accountString;

            var importAccountResult = CoreService.ImportAccount(accountString).WaitResult();
            Assert.AreEqual(Core.ImportAccount.PossibleErrors.AccountExistsAlready,
                CastOrDie(importAccountResult, out Core.ImportAccount.ExpectedError _).Error);
        }

        [TestMethod]
        public void GetRoot() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = CoreService.GetRoot().WaitResult();
            CastOrDie(getRootResult, out Core.GetRoot.Success _);
        }

        [TestMethod]
        public void GetRootNoRoot() {
            var getRootResult = CoreService.GetRoot().WaitResult();
            Assert.AreEqual(Core.GetRoot.PossibleErrors.NoRoot,
                CastOrDie(getRootResult, out Core.GetRoot.ExpectedError _).Error);
        }

        [TestMethod]
        public void ListMetadatas() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var listFileMetadataResult = CoreService.ListMetadatas().WaitResult();
            CastOrDie(listFileMetadataResult, out Core.ListMetadatas.Success _);
        }

        [TestMethod]
        public void GetChildren() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var listFileMetadataResult = CoreService.ListMetadatas().WaitResult();
            CastOrDie(listFileMetadataResult, out Core.ListMetadatas.Success _);
        }

        [TestMethod]
        public void SyncAll() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            CoreService.SyncAll().WaitResult();
            var syncAllResult = CoreService.SyncAll().WaitResult();
            CastOrDie(syncAllResult, out Core.SyncAll.Success _);
        }

        [TestMethod]
        public void SyncAllNoAccount() {
            CoreService.SyncAll().WaitResult();
            var syncAllResult = CoreService.SyncAll().WaitResult();
            Assert.AreEqual(Core.SyncAll.PossibleErrors.NoAccount,
                CastOrDie(syncAllResult, out Core.SyncAll.ExpectedError _).Error);
        }

        [TestMethod]
        public void CreateFile() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = CoreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = CoreService.CreateFile("TestFile", root.Id, FileType.Document).WaitResult();
            CastOrDie(createFileResult, out Core.CreateFile.Success _);
        }

        [TestMethod]
        public void CreateFileNoAccount() {
            var createFileResult = CoreService.CreateFile("TestFile", Guid.NewGuid().ToString(), FileType.Document).WaitResult();
            Assert.AreEqual(Core.CreateFile.PossibleErrors.NoAccount,
                CastOrDie(createFileResult, out Core.CreateFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void CreateFileDocTreatedAsFolder() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = CoreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = CoreService.CreateFile("TestFile", root.Id, FileType.Document).WaitResult();
            var file = CastOrDie(createFileResult, out Core.CreateFile.Success _).newFile;
            
            var createFileResult2 = CoreService.CreateFile("TestFile", file.Id, FileType.Document).WaitResult();
            Assert.AreEqual(Core.CreateFile.PossibleErrors.DocumentTreatedAsFolder,
                CastOrDie(createFileResult2, out Core.CreateFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void CreateFileCouldNotFindAParent() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var createFileResult = CoreService.CreateFile("TestFile", Guid.NewGuid().ToString(), FileType.Document).WaitResult();
            Assert.AreEqual(Core.CreateFile.PossibleErrors.CouldNotFindAParent,
                CastOrDie(createFileResult, out Core.CreateFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void CreateFileFileNameNotAvailable() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = CoreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = CoreService.CreateFile("TestFile", root.Id, FileType.Document).WaitResult();
            CastOrDie(createFileResult, out Core.CreateFile.Success _);

            var createFileResult2 = CoreService.CreateFile("TestFile", root.Id, FileType.Document).WaitResult();
            Assert.AreEqual(Core.CreateFile.PossibleErrors.FileNameNotAvailable,
                CastOrDie(createFileResult2, out Core.CreateFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void CreateFileFileNameContainsSlash() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = CoreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = CoreService.CreateFile("Test/File", root.Id, FileType.Document).WaitResult();
            Assert.AreEqual(Core.CreateFile.PossibleErrors.FileNameContainsSlash,
                CastOrDie(createFileResult, out Core.CreateFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void CreateFileFileNameEmpty() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = CoreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = CoreService.CreateFile("", root.Id, FileType.Document).WaitResult();
            Assert.AreEqual(Core.CreateFile.PossibleErrors.FileNameEmpty,
                CastOrDie(createFileResult, out Core.CreateFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void WriteDoc() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = CoreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = CoreService.CreateFile("TestFile", root.Id, FileType.Document).WaitResult(); 
            var file = CastOrDie(createFileResult, out Core.CreateFile.Success _).newFile;

            var writeDocResult = CoreService.WriteDocument(file.Id, "content").WaitResult();
            CastOrDie(writeDocResult, out Core.WriteDocument.Success _);
        }

        [TestMethod]
        public void WriteDocNoAccount() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = CoreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = CoreService.CreateFile("TestFile", root.Id, FileType.Document).WaitResult(); 
            CastOrDie(createFileResult, out Core.CreateFile.Success _);
            var fileId = ((Core.CreateFile.Success)createFileResult).newFile.Id;
            DeleteAccount();

            var writeDocResult = CoreService.WriteDocument(fileId, "content").WaitResult();
            Assert.AreEqual(Core.WriteDocument.PossibleErrors.NoAccount,
                CastOrDie(writeDocResult, out Core.WriteDocument.ExpectedError _).Error);
        }

        [TestMethod]
        public void ReadDoc() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = CoreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = CoreService.CreateFile("TestFile", root.Id, FileType.Document).WaitResult();
            CastOrDie(createFileResult, out Core.CreateFile.Success _);
            var fileId = ((Core.CreateFile.Success)createFileResult).newFile.Id;

            var readDocResult = CoreService.ReadDocument(fileId).WaitResult();
            CastOrDie(readDocResult, out Core.ReadDocument.Success _);
        }

        [TestMethod]
        public void ReadDocNoAccount() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = CoreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = CoreService.CreateFile("TestFile", root.Id, FileType.Document).WaitResult(); 
            CastOrDie(createFileResult, out Core.CreateFile.Success _);
            var fileId = ((Core.CreateFile.Success)createFileResult).newFile.Id;
            DeleteAccount();

            var readDocResult = CoreService.ReadDocument(fileId).WaitResult();
            Assert.AreEqual(Core.ReadDocument.PossibleErrors.NoAccount,
                CastOrDie(readDocResult, out Core.ReadDocument.ExpectedError _).Error);
        }

        [TestMethod]
        public void RenameFile() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = CoreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = CoreService.CreateFile("TestFile", root.Id, FileType.Document).WaitResult();
            CastOrDie(createFileResult, out Core.CreateFile.Success _);
            var fileId = ((Core.CreateFile.Success)createFileResult).newFile.Id;

            var renameFileResult = CoreService.RenameFile(fileId, "NewTestFile").WaitResult();
            CastOrDie(renameFileResult, out Core.RenameFile.Success _);
        }

        [TestMethod]
        public void MoveFile() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = CoreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = CoreService.CreateFile("TestFile", root.Id, FileType.Document).WaitResult();
            var file = CastOrDie(createFileResult, out Core.CreateFile.Success _).newFile;

            var createFolderResult = CoreService.CreateFile("TestFile2", root.Id, FileType.Folder).WaitResult();
            var folder = CastOrDie(createFolderResult, out Core.CreateFile.Success _).newFile;

            var moveFileResult = CoreService.MoveFile(file.Id, folder.Id).WaitResult();
            CastOrDie(moveFileResult, out Core.MoveFile.Success _);
        }

        [TestMethod]
        public void MoveFileNoAccount() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = CoreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = CoreService.CreateFile("TestFile", root.Id, FileType.Document).WaitResult();
            var file = CastOrDie(createFileResult, out Core.CreateFile.Success _).newFile;

            var createFolderResult = CoreService.CreateFile("TestFile2", root.Id, FileType.Folder).WaitResult();
            var folder = CastOrDie(createFolderResult, out Core.CreateFile.Success _).newFile;
            DeleteAccount();

            var moveFileResult = CoreService.MoveFile(file.Id, folder.Id).WaitResult();
            Assert.AreEqual(Core.MoveFile.PossibleErrors.NoAccount,
                CastOrDie(moveFileResult, out Core.MoveFile.ExpectedError _).Error);
        }
    }
}