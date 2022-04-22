using Core;
using lockbook;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using Newtonsoft.Json;
using Newtonsoft.Json.Converters;
using System;
using System.Collections.Generic;
using System.IO;
using System.Threading;

namespace test {
    [TestClass]
    public class CoreServiceTest {
        readonly string apiUrl = Environment.GetEnvironmentVariable("API_URL");

        public string RandomUsername() {
            return "testUsername" + Guid.NewGuid().ToString().Replace("-", "");
        }

        public string RandomLockbookDir() {
            return "C:\\Temp\\.lockbook\\" + Guid.NewGuid().ToString().Replace("-", "");
        }

        public CoreService NewCoreService() {
            var result = new CoreService();
            switch (result.Init(RandomLockbookDir(), false).WaitResult()) {
                case Core.Init.Success:
                    break;
                case Core.Init.UnexpectedError error:
                    throw new Exception("Unexpected error while initializing core: " + error.ErrorMessage);
            }
            return result;
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

        [ClassCleanup]
        public static void DeleteData() {
            try {
                Directory.Delete("C:\\Temp\\.lockbook", true);
            } catch (DirectoryNotFoundException) { }
        }

        [TestMethod]
        public void AccountExistsFalse() {
            var coreService = NewCoreService();
            Assert.IsFalse(coreService.AccountExists().WaitResult());
        }

        [TestMethod]
        public void AccountExistsTrue() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getAccountResult = coreService.GetAccount().WaitResult();
            CastOrDie(getAccountResult, out Core.GetAccount.Success _);
            Assert.IsTrue(coreService.AccountExists().WaitResult());
        }

        [TestMethod]
        public void CreateAccountSuccess() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);
        }

        [TestMethod]
        public void CreateAccountAccountExistsAlready() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var username2 = RandomUsername();
            var createAccountResult2 = coreService.CreateAccount(username2, apiUrl).WaitResult();
            Assert.AreEqual(Core.CreateAccount.PossibleErrors.AccountExistsAlready,
                CastOrDie(createAccountResult2, out Core.CreateAccount.ExpectedError _).Error);
        }

        [TestMethod]
        public void CreateAccountUsernameTaken() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var syncResult = coreService.SyncAll().WaitResult();
            CastOrDie(syncResult, out Core.SyncAll.Success _);

            coreService = NewCoreService();

            var createAccountResult2 = coreService.CreateAccount(username, apiUrl).WaitResult();
            Assert.AreEqual(Core.CreateAccount.PossibleErrors.UsernameTaken,
                CastOrDie(createAccountResult2, out Core.CreateAccount.ExpectedError _).Error);
        }

        [TestMethod]
        public void CreateAccountInvalidUsername() {
            var coreService = NewCoreService();
            var username = "not! a! valid! username!";
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            Assert.AreEqual(Core.CreateAccount.PossibleErrors.InvalidUsername,
                CastOrDie(createAccountResult, out Core.CreateAccount.ExpectedError _).Error);
        }

        [TestMethod]
        public void GetAccount() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getAccountResult = coreService.GetAccount().WaitResult();
            CastOrDie(getAccountResult, out Core.GetAccount.Success _);
        }

        [TestMethod]
        public void GetAccountNoAccount() {
            var coreService = NewCoreService();
            var getAccountResult = coreService.GetAccount().WaitResult();
            Assert.AreEqual(Core.GetAccount.PossibleErrors.NoAccount,
               CastOrDie(getAccountResult, out Core.GetAccount.ExpectedError _).Error);
        }

        [TestMethod]
        public void ExportAccount() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var exportAccountResult = coreService.ExportAccount().WaitResult();
            CastOrDie(exportAccountResult, out Core.ExportAccount.Success _);
        }

        [TestMethod]
        public void ExportAccountNoAccount() {
            var coreService = NewCoreService();
            var exportAccountResult = coreService.ExportAccount().WaitResult();
            Assert.AreEqual(Core.ExportAccount.PossibleErrors.NoAccount,
               CastOrDie(exportAccountResult, out Core.ExportAccount.ExpectedError _).Error);
        }

        [TestMethod]
        public void ImportAccount() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var exportAccountResult = coreService.ExportAccount().WaitResult();
            var accountString = CastOrDie(exportAccountResult, out Core.ExportAccount.Success _).accountString;

            coreService = NewCoreService();

            var importAccountResult = coreService.ImportAccount(accountString).WaitResult();
            CastOrDie(importAccountResult, out Core.ImportAccount.Success _);
        }

        [TestMethod]
        public void ImportAccountAccountStringCorrupted() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var accountString = "#######!!@$@%";

            coreService = NewCoreService();

            var importAccountResult = coreService.ImportAccount(accountString).WaitResult();
            Assert.AreEqual(Core.ImportAccount.PossibleErrors.AccountStringCorrupted,
                CastOrDie(importAccountResult, out Core.ImportAccount.ExpectedError _).Error);
        }

        [TestMethod]
        public void ImportAccountAccountExistsAlready() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var exportAccountResult = coreService.ExportAccount().WaitResult();
            var accountString = CastOrDie(exportAccountResult, out Core.ExportAccount.Success _).accountString;

            var importAccountResult = coreService.ImportAccount(accountString).WaitResult();
            Assert.AreEqual(Core.ImportAccount.PossibleErrors.AccountExistsAlready,
                CastOrDie(importAccountResult, out Core.ImportAccount.ExpectedError _).Error);
        }

        [TestMethod]
        public void GetRoot() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = coreService.GetRoot().WaitResult();
            CastOrDie(getRootResult, out Core.GetRoot.Success _);
        }

        [TestMethod]
        public void GetRootNoRoot() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var exportAccountResult = coreService.ExportAccount().WaitResult();
            var accountString = CastOrDie(exportAccountResult, out Core.ExportAccount.Success _).accountString;

            coreService = NewCoreService();

            var importAccountResult = coreService.ImportAccount(accountString).WaitResult();
            CastOrDie(importAccountResult, out Core.ImportAccount.Success _);
            
            var getRootResult = coreService.GetRoot().WaitResult();
            Assert.AreEqual(Core.GetRoot.PossibleErrors.NoRoot,
                CastOrDie(getRootResult, out Core.GetRoot.ExpectedError _).Error);
        }

        [TestMethod]
        public void ListMetadatas() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var listFileMetadataResult = coreService.ListMetadatas().WaitResult();
            CastOrDie(listFileMetadataResult, out Core.ListMetadatas.Success _);
        }

        [TestMethod]
        public void GetChildren() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = coreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var getChildrenResult = coreService.GetChildren(root.id).WaitResult();
            CastOrDie(getChildrenResult, out Core.GetChildren.Success _);
        }

        [TestMethod]
        public void SyncAll() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var syncAllResult = coreService.SyncAll().WaitResult();
            CastOrDie(syncAllResult, out Core.SyncAll.Success _);
        }

        [TestMethod]
        public void SyncAllNoAccount() {
            var coreService = NewCoreService();
            var syncAllResult = coreService.SyncAll().WaitResult();
            Assert.AreEqual(Core.SyncAll.PossibleErrors.NoAccount,
                CastOrDie(syncAllResult, out Core.SyncAll.ExpectedError _).Error);
        }

        [TestMethod]
        public void CalculateWork() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var calculateWorkResult = coreService.CalculateWork().WaitResult();
            CastOrDie(calculateWorkResult, out Core.CalculateWork.Success _);
        }

        [TestMethod]
        public void CalculateWorkNoAccount() {
            var coreService = NewCoreService();
            var calculateWorkResult = coreService.CalculateWork().WaitResult();
            Assert.AreEqual(Core.CalculateWork.PossibleErrors.NoAccount,
                CastOrDie(calculateWorkResult, out Core.CalculateWork.ExpectedError _).Error);
        }

        [TestMethod]
        public void GetLastSynced() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getLastSyncedResult = coreService.GetLastSynced().WaitResult();
            CastOrDie(getLastSyncedResult, out Core.GetLastSynced.Success _);
        }

        [TestMethod]
        public void GetUsage() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getUsageResult = coreService.GetUsage().WaitResult();
            CastOrDie(getUsageResult, out Core.GetUsage.Success _);
        }

        [TestMethod]
        public void GetUsageNoAccount() {
            var coreService = NewCoreService();
            var getUsageResult = coreService.GetUsage().WaitResult();
            Assert.AreEqual(Core.GetUsage.PossibleErrors.NoAccount,
                CastOrDie(getUsageResult, out Core.GetUsage.ExpectedError _).Error);
        }

        [TestMethod]
        public void CreateFile() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = coreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = coreService.CreateFile("TestFile", root.id, FileType.Document).WaitResult();
            CastOrDie(createFileResult, out Core.CreateFile.Success _);
        }

        [TestMethod]
        public void CreateFileNoAccount() {
            var coreService = NewCoreService();
            var createFileResult = coreService.CreateFile("TestFile", Guid.NewGuid().ToString(), FileType.Document).WaitResult();
            Assert.AreEqual(Core.CreateFile.PossibleErrors.NoAccount,
                CastOrDie(createFileResult, out Core.CreateFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void CreateFileDocTreatedAsFolder() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = coreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = coreService.CreateFile("TestFile", root.id, FileType.Document).WaitResult();
            var file = CastOrDie(createFileResult, out Core.CreateFile.Success _).newFile;

            var createFileResult2 = coreService.CreateFile("TestFile", file.id, FileType.Document).WaitResult();
            Assert.AreEqual(Core.CreateFile.PossibleErrors.DocumentTreatedAsFolder,
                CastOrDie(createFileResult2, out Core.CreateFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void CreateFileFileNameNotAvailable() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = coreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = coreService.CreateFile("TestFile", root.id, FileType.Document).WaitResult();
            CastOrDie(createFileResult, out Core.CreateFile.Success _);

            var createFileResult2 = coreService.CreateFile("TestFile", root.id, FileType.Document).WaitResult();
            Assert.AreEqual(Core.CreateFile.PossibleErrors.FileNameNotAvailable,
                CastOrDie(createFileResult2, out Core.CreateFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void CreateFileFileNameContainsSlash() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = coreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = coreService.CreateFile("Test/File", root.id, FileType.Document).WaitResult();
            Assert.AreEqual(Core.CreateFile.PossibleErrors.FileNameContainsSlash,
                CastOrDie(createFileResult, out Core.CreateFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void CreateFileFileNameEmpty() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = coreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = coreService.CreateFile("", root.id, FileType.Document).WaitResult();
            Assert.AreEqual(Core.CreateFile.PossibleErrors.FileNameEmpty,
                CastOrDie(createFileResult, out Core.CreateFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void WriteDoc() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = coreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = coreService.CreateFile("TestFile", root.id, FileType.Document).WaitResult();
            var file = CastOrDie(createFileResult, out Core.CreateFile.Success _).newFile;

            var writeDocResult = coreService.WriteDocument(file.id, "test content").WaitResult();
            CastOrDie(writeDocResult, out Core.WriteDocument.Success _);
        }

        [TestMethod]
        public void WriteDocNoAccount() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = coreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = coreService.CreateFile("TestFile", root.id, FileType.Document).WaitResult();
            CastOrDie(createFileResult, out Core.CreateFile.Success _);
            var fileId = ((Core.CreateFile.Success)createFileResult).newFile.id;

            coreService = NewCoreService();

            var writeDocResult = coreService.WriteDocument(fileId, "content").WaitResult();
            Assert.AreEqual(Core.WriteDocument.PossibleErrors.NoAccount,
                CastOrDie(writeDocResult, out Core.WriteDocument.ExpectedError _).Error);
        }

        [TestMethod]
        public void ReadDoc() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = coreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = coreService.CreateFile("TestFile", root.id, FileType.Document).WaitResult();
            CastOrDie(createFileResult, out Core.CreateFile.Success _);
            var fileId = ((Core.CreateFile.Success)createFileResult).newFile.id;

            var writeDocResult = coreService.WriteDocument(fileId, "test content").WaitResult();
            CastOrDie(writeDocResult, out Core.WriteDocument.Success _);

            var readDocResult = coreService.ReadDocument(fileId).WaitResult();
            Assert.AreEqual("test content",
                CastOrDie(readDocResult, out Core.ReadDocument.Success _).content);
        }

        [TestMethod]
        public void ReadDocNoAccount() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = coreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = coreService.CreateFile("TestFile", root.id, FileType.Document).WaitResult();
            CastOrDie(createFileResult, out Core.CreateFile.Success _);
            var fileId = ((Core.CreateFile.Success)createFileResult).newFile.id;

            coreService = NewCoreService();

            var readDocResult = coreService.ReadDocument(fileId).WaitResult();
            Assert.AreEqual(Core.ReadDocument.PossibleErrors.NoAccount,
                CastOrDie(readDocResult, out Core.ReadDocument.ExpectedError _).Error);
        }

        [TestMethod]
        public void ReadDocTreatedFolderAsDocument() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = coreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = coreService.CreateFile("TestFile", root.id, FileType.Folder).WaitResult();
            CastOrDie(createFileResult, out Core.CreateFile.Success _);
            var fileId = ((Core.CreateFile.Success)createFileResult).newFile.id;

            var readDocResult = coreService.ReadDocument(fileId).WaitResult();
            Assert.AreEqual(Core.ReadDocument.PossibleErrors.TreatedFolderAsDocument,
                CastOrDie(readDocResult, out Core.ReadDocument.ExpectedError _).Error);
        }

        [TestMethod]
        public void ReadDocFileDoesNotExist() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var readDocResult = coreService.ReadDocument(Guid.NewGuid().ToString()).WaitResult();
            Assert.AreEqual(Core.ReadDocument.PossibleErrors.FileDoesNotExist,
                CastOrDie(readDocResult, out Core.ReadDocument.ExpectedError _).Error);
        }

        [TestMethod]
        public void RenameFile() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = coreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = coreService.CreateFile("TestFile", root.id, FileType.Document).WaitResult();
            CastOrDie(createFileResult, out Core.CreateFile.Success _);
            var fileId = ((Core.CreateFile.Success)createFileResult).newFile.id;

            var renameFileResult = coreService.RenameFile(fileId, "NewTestFile").WaitResult();
            CastOrDie(renameFileResult, out Core.RenameFile.Success _);
        }

        [TestMethod]
        public void MoveFile() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = coreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = coreService.CreateFile("TestFile", root.id, FileType.Document).WaitResult();
            var file = CastOrDie(createFileResult, out Core.CreateFile.Success _).newFile;

            var createFolderResult = coreService.CreateFile("TestFile2", root.id, FileType.Folder).WaitResult();
            var folder = CastOrDie(createFolderResult, out Core.CreateFile.Success _).newFile;

            var moveFileResult = coreService.MoveFile(file.id, folder.id).WaitResult();
            CastOrDie(moveFileResult, out Core.MoveFile.Success _);
        }

        [TestMethod]
        public void MoveFileNoAccount() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = coreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = coreService.CreateFile("TestFile", root.id, FileType.Document).WaitResult();
            var file = CastOrDie(createFileResult, out Core.CreateFile.Success _).newFile;

            var createFolderResult = coreService.CreateFile("TestFile2", root.id, FileType.Folder).WaitResult();
            var folder = CastOrDie(createFolderResult, out Core.CreateFile.Success _).newFile;

            coreService = NewCoreService();

            var moveFileResult = coreService.MoveFile(file.id, folder.id).WaitResult();
            Assert.AreEqual(Core.MoveFile.PossibleErrors.NoAccount,
                CastOrDie(moveFileResult, out Core.MoveFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void MoveFileFileDoesNotExist() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = coreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var moveFileResult = coreService.MoveFile(Guid.NewGuid().ToString(), root.id).WaitResult();
            Assert.AreEqual(Core.MoveFile.PossibleErrors.FileDoesNotExist,
                CastOrDie(moveFileResult, out Core.MoveFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void MoveFileDocumentTreatedAsFolder() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = coreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = coreService.CreateFile("TestFile", root.id, FileType.Document).WaitResult();
            var file = CastOrDie(createFileResult, out Core.CreateFile.Success _).newFile;

            var createFileResult2 = coreService.CreateFile("TestFile2", root.id, FileType.Document).WaitResult();
            var file2 = CastOrDie(createFileResult2, out Core.CreateFile.Success _).newFile;

            var moveFileResult = coreService.MoveFile(file.id, file2.id).WaitResult();
            Assert.AreEqual(Core.MoveFile.PossibleErrors.DocumentTreatedAsFolder,
                CastOrDie(moveFileResult, out Core.MoveFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void MoveFileTargetParentHasChildNamedThat() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = coreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = coreService.CreateFile("TestFile", root.id, FileType.Document).WaitResult();
            var file = CastOrDie(createFileResult, out Core.CreateFile.Success _).newFile;

            var createFolderResult = coreService.CreateFile("TestFile2", root.id, FileType.Folder).WaitResult();
            var folder = CastOrDie(createFolderResult, out Core.CreateFile.Success _).newFile;

            var createFileResult2 = coreService.CreateFile("TestFile", folder.id, FileType.Document).WaitResult();
            CastOrDie(createFileResult2, out Core.CreateFile.Success _);

            var moveFileResult = coreService.MoveFile(file.id, folder.id).WaitResult();
            Assert.AreEqual(Core.MoveFile.PossibleErrors.TargetParentHasChildNamedThat,
                CastOrDie(moveFileResult, out Core.MoveFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void MoveFileTargetParentDoesNotExist() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = coreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = coreService.CreateFile("TestFile", root.id, FileType.Document).WaitResult();
            var file = CastOrDie(createFileResult, out Core.CreateFile.Success _).newFile;

            var moveFileResult = coreService.MoveFile(file.id, Guid.NewGuid().ToString()).WaitResult();
            Assert.AreEqual(Core.MoveFile.PossibleErrors.TargetParentDoesNotExist,
                CastOrDie(moveFileResult, out Core.MoveFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void MoveFileCannotMoveRoot() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = coreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFolderResult = coreService.CreateFile("TestFile2", root.id, FileType.Folder).WaitResult();
            var folder = CastOrDie(createFolderResult, out Core.CreateFile.Success _).newFile;

            var moveFileResult = coreService.MoveFile(root.id, folder.id).WaitResult();
            Assert.AreEqual(Core.MoveFile.PossibleErrors.CannotMoveRoot,
               CastOrDie(moveFileResult, out Core.MoveFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void MoveFileIntoItself() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = coreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFolderResult1 = coreService.CreateFile("TestFile", root.id, FileType.Folder).WaitResult();
            var folder1 = CastOrDie(createFolderResult1, out Core.CreateFile.Success _).newFile;

            var createFolderResult2 = coreService.CreateFile("TestFile2", folder1.id, FileType.Folder).WaitResult();
            var folder2 = CastOrDie(createFolderResult2, out Core.CreateFile.Success _).newFile;

            var moveFileResult = coreService.MoveFile(folder1.id, folder2.id).WaitResult();
            Assert.AreEqual(Core.MoveFile.PossibleErrors.FolderMovedIntoItself,
               CastOrDie(moveFileResult, out Core.MoveFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void DeleteFile() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = coreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = coreService.CreateFile("TestFile", root.id, FileType.Document).WaitResult();
            CastOrDie(createFileResult, out Core.CreateFile.Success _);
            var fileId = ((Core.CreateFile.Success)createFileResult).newFile.id;

            var deleteFileResult = coreService.DeleteFile(fileId).WaitResult();
            CastOrDie(deleteFileResult, out Core.DeleteFile.Success _);
        }

        [TestMethod]
        public void DeleteFileDoesNotExist() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var deleteFileResult = coreService.DeleteFile(Guid.NewGuid().ToString()).WaitResult();
            Assert.AreEqual(Core.DeleteFile.PossibleErrors.FileDoesNotExist,
                CastOrDie(deleteFileResult, out Core.DeleteFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void DeleteFileCannotDeleteRoot() {
            var coreService = NewCoreService();
            var username = RandomUsername();
            var createAccountResult = coreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = coreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var deleteFileResult = coreService.DeleteFile(root.id).WaitResult();
            Assert.AreEqual(Core.DeleteFile.PossibleErrors.CannotDeleteRoot,
                CastOrDie(deleteFileResult, out Core.DeleteFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void GetVariants() {
            var coreService = NewCoreService();
            var typeMap = new Dictionary<string, Type> {
                {"AccountExportError", typeof(Core.ExportAccount.PossibleErrors)},
                {"CalculateWorkError", typeof(Core.CalculateWork.PossibleErrors)},
                {"CreateAccountError", typeof(Core.CreateAccount.PossibleErrors)},
                {"CreateFileAtPathError", typeof(Core.CreateFileAtPath.PossibleErrors)},
                {"CreateFileError", typeof(Core.CreateFile.PossibleErrors)},
                {"FileDeleteError", typeof(Core.DeleteFile.PossibleErrors)},
                {"GetAccountError", typeof(Core.GetAccount.PossibleErrors)},
                {"GetChildrenError", typeof(Core.GetChildren.PossibleErrors)},
                // {"GetFileByIdError", typeof(Core.???.PossibleErrors)},
                {"GetFileByPathError", typeof(Core.GetByPath.PossibleErrors)},
                {"GetLastSyncedError", typeof(Core.GetLastSynced.PossibleErrors)},
                {"GetRootError", typeof(Core.GetRoot.PossibleErrors)},
                {"GetUsageError", typeof(Core.GetUsage.PossibleErrors)},
                {"ImportError", typeof(Core.ImportAccount.PossibleErrors)},
                // {"InsertFileError", typeof(Core.???.PossibleErrors)},
                {"ListMetadatasError", typeof(Core.ListMetadatas.PossibleErrors)},
                {"ListPathsError", typeof(Core.ListPaths.PossibleErrors)},
                {"MoveFileError", typeof(Core.MoveFile.PossibleErrors)},
                {"ReadDocumentError", typeof(Core.ReadDocument.PossibleErrors)},
                {"RenameFileError", typeof(Core.RenameFile.PossibleErrors)},
                {"SyncAllError", typeof(Core.SyncAll.PossibleErrors)},
                {"WriteToDocumentError", typeof(Core.WriteDocument.PossibleErrors)},
                {"GetDrawingError", typeof(Core.GetDrawing.PossibleErrors)},
                {"SaveDrawingError", typeof(Core.SaveDrawing.PossibleErrors)},
                {"ExportDrawingError", typeof(Core.ExportDrawing.PossibleErrors)},
                // {"ExportDrawingToDiskError", typeof(Core.???.PossibleErrors)},
                // {"SaveDocumentToDiskError", typeof(Core.???.PossibleErrors)},
            };

            var variants = coreService.GetVariants().WaitResult();

            foreach(var kvp in variants) {
                if(kvp.Key == "GetFileByIdError" || kvp.Key == "InsertFileError" || kvp.Key == "ExportDrawingToDiskError" || kvp.Key == "SaveDocumentToDiskError") {
                    continue; // these endpoints, and therefore these errors, are not used by this client
                }
                var enumType = typeMap[kvp.Key];
                foreach(var variant in kvp.Value) {
                    if (!Enum.TryParse(enumType, variant, out var _)) {
                        Assert.Fail("Enum variant from core not present in c#. enum=" + kvp.Key + " variant=" + variant);
                    }
                }
            }
        }
    }
}