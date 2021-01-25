using Core;
using lockbook;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using Newtonsoft.Json;
using Newtonsoft.Json.Converters;
using System;
using System.IO;
using System.Threading.Tasks;

namespace test {
    [TestClass]
    public class CoreServiceTest {
        const string lockbookDir = "C:\\Temp\\.lockbook"; // todo: find a more suitable location
        CoreService CoreService = new CoreService(lockbookDir);
        string apiUrl = Environment.GetEnvironmentVariable("API_URL");

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
        public void GetDbStateEmpty() {
            var getDbStateResult = CoreService.GetDbState().WaitResult();
            Assert.AreEqual(DbState.Empty,
                CastOrDie(getDbStateResult, out Core.GetDbState.Success _).dbState);
        }

        [TestMethod]
        public void GetDbStateReady() {
            var getDbStateResult = CoreService.GetDbState().WaitResult();
            Assert.AreEqual(DbState.Empty,
                CastOrDie(getDbStateResult, out Core.GetDbState.Success _).dbState);

            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getDbStateResult2 = CoreService.GetDbState().WaitResult();
            Assert.AreEqual(DbState.ReadyToUse,
                CastOrDie(getDbStateResult2, out Core.GetDbState.Success _).dbState);
        }

        [TestMethod]
        public void MigrateDb() {
            // needs to be done first
            var getDbStateResult = CoreService.GetDbState().WaitResult();
            CastOrDie(getDbStateResult, out Core.GetDbState.Success _);

            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

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
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getAccountResult = CoreService.GetAccount().WaitResult();
            CastOrDie(getAccountResult, out Core.GetAccount.Success _);
            Assert.IsTrue(CoreService.AccountExists().WaitResult());
        }

        [TestMethod]
        public void CreateAccountSuccess() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);
        }

        [TestMethod]
        public void CreateAccountAccountExistsAlready() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var username2 = RandomUsername();
            var createAccountResult2 = CoreService.CreateAccount(username2, apiUrl).WaitResult();
            Assert.AreEqual(Core.CreateAccount.PossibleErrors.AccountExistsAlready,
                CastOrDie(createAccountResult2, out Core.CreateAccount.ExpectedError _).Error);
        }

        [TestMethod]
        public void CreateAccountUsernameTaken() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var syncResult = CoreService.SyncAll().WaitResult();
            CastOrDie(syncResult, out Core.SyncAll.Success _);

            Directory.Delete(lockbookDir, true);

            var createAccountResult2 = CoreService.CreateAccount(username, apiUrl).WaitResult();
            Assert.AreEqual(Core.CreateAccount.PossibleErrors.UsernameTaken,
                CastOrDie(createAccountResult2, out Core.CreateAccount.ExpectedError _).Error);
        }

        [TestMethod]
        public void CreateAccountInvalidUsername() {
            var username = "not! a! valid! username!";
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            Assert.AreEqual(Core.CreateAccount.PossibleErrors.InvalidUsername,
                CastOrDie(createAccountResult, out Core.CreateAccount.ExpectedError _).Error);
        }

        [TestMethod]
        public void GetAccount() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
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
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
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
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
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
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
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
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
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
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
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
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var listFileMetadataResult = CoreService.ListMetadatas().WaitResult();
            CastOrDie(listFileMetadataResult, out Core.ListMetadatas.Success _);
        }

        [TestMethod]
        public void GetChildren() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = CoreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var getChildrenResult = CoreService.GetChildren(root.Id).WaitResult();
            CastOrDie(getChildrenResult, out Core.GetChildren.Success _);
        }

        [TestMethod]
        public void SyncAll() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var syncAllResult = CoreService.SyncAll().WaitResult();
            CastOrDie(syncAllResult, out Core.SyncAll.Success _);
        }

        [TestMethod]
        public void SyncAllNoAccount() {
            var syncAllResult = CoreService.SyncAll().WaitResult();
            Assert.AreEqual(Core.SyncAll.PossibleErrors.NoAccount,
                CastOrDie(syncAllResult, out Core.SyncAll.ExpectedError _).Error);
        }

        [TestMethod]
        public void CalculateWork() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var calculateWorkResult = CoreService.CalculateWork().WaitResult();
            CastOrDie(calculateWorkResult, out Core.CalculateWork.Success _);
        }

        [TestMethod]
        public void CalculateWorkNoAccount() {
            var calculateWorkResult = CoreService.CalculateWork().WaitResult();
            Assert.AreEqual(Core.CalculateWork.PossibleErrors.NoAccount,
                CastOrDie(calculateWorkResult, out Core.CalculateWork.ExpectedError _).Error);
        }

        [TestMethod]
        public void ExecuteWork() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = CoreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = CoreService.CreateFile("TestFile", root.Id, FileType.Document).WaitResult();
            CastOrDie(createFileResult, out Core.CreateFile.Success _);

            var calculateWorkResult = CoreService.CalculateWork().WaitResult();
            var work = CastOrDie(calculateWorkResult, out Core.CalculateWork.Success _).workCalculated;

            var executeWorkResult = ((Task<Core.ExecuteWork.IResult>)CoreService.ExecuteWork(JsonConvert.SerializeObject(work.workUnits[0]))).WaitResult();
            CastOrDie(executeWorkResult, out Core.ExecuteWork.Success _);
        }

        [TestMethod]
        public void GetLastSynced() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getLastSyncedResult = CoreService.GetLastSynced().WaitResult();
            CastOrDie(getLastSyncedResult, out Core.GetLastSynced.Success _);
        }

        [TestMethod]
        public void GetLastSyncedHumanString() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getLastSyncedResult = CoreService.GetLastSyncedHumanString().WaitResult();
            CastOrDie(getLastSyncedResult, out Core.GetLastSyncedHumanString.Success _);
        }

        [TestMethod]
        public void SetLastSynced() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var setLastSyncedResult = CoreService.SetLastSynced(420).WaitResult();
            CastOrDie(setLastSyncedResult, out Core.SetLastSynced.Success _);
        }

        [TestMethod]
        public void GetUsage() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getUsageResult = CoreService.GetUsage().WaitResult();
            CastOrDie(getUsageResult, out Core.GetUsage.Success _);
        }

        [TestMethod]
        public void GetUsageHumanString() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getUsageResult = CoreService.GetUsageHumanString().WaitResult();
            CastOrDie(getUsageResult, out Core.GetUsageHumanString.Success _);
        }

        [TestMethod]
        public void GetUsageNoAccount() {
            var getUsageResult = CoreService.GetUsage().WaitResult();
            Assert.AreEqual(Core.GetUsage.PossibleErrors.NoAccount,
                CastOrDie(getUsageResult, out Core.GetUsage.ExpectedError _).Error);
        }

        [TestMethod]
        public void CreateFile() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
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
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
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
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var createFileResult = CoreService.CreateFile("TestFile", Guid.NewGuid().ToString(), FileType.Document).WaitResult();
            Assert.AreEqual(Core.CreateFile.PossibleErrors.CouldNotFindAParent,
                CastOrDie(createFileResult, out Core.CreateFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void CreateFileFileNameNotAvailable() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
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
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
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
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
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
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = CoreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = CoreService.CreateFile("TestFile", root.Id, FileType.Document).WaitResult();
            var file = CastOrDie(createFileResult, out Core.CreateFile.Success _).newFile;

            var writeDocResult = CoreService.WriteDocument(file.Id, "test content").WaitResult();
            CastOrDie(writeDocResult, out Core.WriteDocument.Success _);
        }

        [TestMethod]
        public void WriteDocNoAccount() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
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
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = CoreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = CoreService.CreateFile("TestFile", root.Id, FileType.Document).WaitResult();
            CastOrDie(createFileResult, out Core.CreateFile.Success _);
            var fileId = ((Core.CreateFile.Success)createFileResult).newFile.Id;

            var writeDocResult = CoreService.WriteDocument(fileId, "test content").WaitResult();
            CastOrDie(writeDocResult, out Core.WriteDocument.Success _);

            var readDocResult = CoreService.ReadDocument(fileId).WaitResult();
            Assert.AreEqual("test content",
                CastOrDie(readDocResult, out Core.ReadDocument.Success _).content);
        }

        [TestMethod]
        public void ReadDocNoAccount() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
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
        public void ReadDocTreatedFolderAsDocument() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = CoreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = CoreService.CreateFile("TestFile", root.Id, FileType.Folder).WaitResult();
            CastOrDie(createFileResult, out Core.CreateFile.Success _);
            var fileId = ((Core.CreateFile.Success)createFileResult).newFile.Id;

            var readDocResult = CoreService.ReadDocument(fileId).WaitResult();
            Assert.AreEqual(Core.ReadDocument.PossibleErrors.TreatedFolderAsDocument,
                CastOrDie(readDocResult, out Core.ReadDocument.ExpectedError _).Error);
        }

        [TestMethod]
        public void ReadDocFileDoesNotExist() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var readDocResult = CoreService.ReadDocument(Guid.NewGuid().ToString()).WaitResult();
            Assert.AreEqual(Core.ReadDocument.PossibleErrors.FileDoesNotExist,
                CastOrDie(readDocResult, out Core.ReadDocument.ExpectedError _).Error);
        }

        [TestMethod]
        public void RenameFile() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
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
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
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
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
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

        [TestMethod]
        public void MoveFileFileDoesNotExist() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = CoreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var moveFileResult = CoreService.MoveFile(Guid.NewGuid().ToString(), root.Id).WaitResult();
            Assert.AreEqual(Core.MoveFile.PossibleErrors.FileDoesNotExist,
                CastOrDie(moveFileResult, out Core.MoveFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void MoveFileDocumentTreatedAsFolder() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = CoreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = CoreService.CreateFile("TestFile", root.Id, FileType.Document).WaitResult();
            var file = CastOrDie(createFileResult, out Core.CreateFile.Success _).newFile;

            var createFileResult2 = CoreService.CreateFile("TestFile2", root.Id, FileType.Document).WaitResult();
            var file2 = CastOrDie(createFileResult2, out Core.CreateFile.Success _).newFile;

            var moveFileResult = CoreService.MoveFile(file.Id, file2.Id).WaitResult();
            Assert.AreEqual(Core.MoveFile.PossibleErrors.DocumentTreatedAsFolder,
                CastOrDie(moveFileResult, out Core.MoveFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void MoveFileTargetParentHasChildNamedThat() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = CoreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = CoreService.CreateFile("TestFile", root.Id, FileType.Document).WaitResult();
            var file = CastOrDie(createFileResult, out Core.CreateFile.Success _).newFile;

            var createFolderResult = CoreService.CreateFile("TestFile2", root.Id, FileType.Folder).WaitResult();
            var folder = CastOrDie(createFolderResult, out Core.CreateFile.Success _).newFile;

            var createFileResult2 = CoreService.CreateFile("TestFile", folder.Id, FileType.Document).WaitResult();
            CastOrDie(createFileResult2, out Core.CreateFile.Success _);

            var moveFileResult = CoreService.MoveFile(file.Id, folder.Id).WaitResult();
            Assert.AreEqual(Core.MoveFile.PossibleErrors.TargetParentHasChildNamedThat,
                CastOrDie(moveFileResult, out Core.MoveFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void MoveFileTargetParentDoesNotExist() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = CoreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = CoreService.CreateFile("TestFile", root.Id, FileType.Document).WaitResult();
            var file = CastOrDie(createFileResult, out Core.CreateFile.Success _).newFile;

            var moveFileResult = CoreService.MoveFile(file.Id, Guid.NewGuid().ToString()).WaitResult();
            Assert.AreEqual(Core.MoveFile.PossibleErrors.TargetParentDoesNotExist,
                CastOrDie(moveFileResult, out Core.MoveFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void MoveFileCannotMoveRoot() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = CoreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFolderResult = CoreService.CreateFile("TestFile2", root.Id, FileType.Folder).WaitResult();
            var folder = CastOrDie(createFolderResult, out Core.CreateFile.Success _).newFile;

            var moveFileResult = CoreService.MoveFile(root.Id, folder.Id).WaitResult();
            Assert.AreEqual(Core.MoveFile.PossibleErrors.CannotMoveRoot,
               CastOrDie(moveFileResult, out Core.MoveFile.ExpectedError _).Error);
        }


        [TestMethod]
        public void MoveFileIntoItself() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = CoreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFolderResult1 = CoreService.CreateFile("TestFile", root.Id, FileType.Folder).WaitResult();
            var folder1 = CastOrDie(createFolderResult1, out Core.CreateFile.Success _).newFile;

            var createFolderResult2 = CoreService.CreateFile("TestFile2", folder1.Id, FileType.Folder).WaitResult();
            var folder2 = CastOrDie(createFolderResult2, out Core.CreateFile.Success _).newFile;

            var moveFileResult = CoreService.MoveFile(folder1.Id, folder2.Id).WaitResult();
            Assert.AreEqual(Core.MoveFile.PossibleErrors.FolderMovedIntoItself,
               CastOrDie(moveFileResult, out Core.MoveFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void DeleteFile() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = CoreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var createFileResult = CoreService.CreateFile("TestFile", root.Id, FileType.Document).WaitResult();
            CastOrDie(createFileResult, out Core.CreateFile.Success _);
            var fileId = ((Core.CreateFile.Success)createFileResult).newFile.Id;

            var deleteFileResult = CoreService.DeleteFile(fileId).WaitResult();
            CastOrDie(deleteFileResult, out Core.DeleteFile.Success _);
        }

        [TestMethod]
        public void DeleteFileDoesNotExist() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var deleteFileResult = CoreService.DeleteFile(Guid.NewGuid().ToString()).WaitResult();
            Assert.AreEqual(Core.DeleteFile.PossibleErrors.FileDoesNotExist,
                CastOrDie(deleteFileResult, out Core.DeleteFile.ExpectedError _).Error);
        }

        [TestMethod]
        public void DeleteFileCannotDeleteRoot() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username, apiUrl).WaitResult();
            CastOrDie(createAccountResult, out Core.CreateAccount.Success _);

            var getRootResult = CoreService.GetRoot().WaitResult();
            var root = CastOrDie(getRootResult, out Core.GetRoot.Success _).root;

            var deleteFileResult = CoreService.DeleteFile(root.Id).WaitResult();
            Assert.AreEqual(Core.DeleteFile.PossibleErrors.CannotDeleteRoot,
                CastOrDie(deleteFileResult, out Core.DeleteFile.ExpectedError _).Error);
        }
    }
}