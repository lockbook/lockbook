using Core;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Threading.Tasks;

namespace lockbook {
    public static class Extensions {
        public static T WaitResult<T>(this Task<T> task) {
            task.Wait();
            return task.Result;
        }
    }

    public class CoreService {
        public IntPtr path;

        public CoreService(string path) {
            this.path = ToPtr(path);
        }

        private static Mutex coreMutex = new Mutex();

        [DllImport("lockbook_core")]
        private unsafe static extern void release_pointer(IntPtr str_pointer);

        [DllImport("lockbook_core")]
        private static extern void init_logger_safely(IntPtr writeable_path);

        [DllImport("lockbook_core")]
        private static extern IntPtr get_db_state(IntPtr writeable_path);

        [DllImport("lockbook_core")]
        private static extern IntPtr migrate_db(IntPtr writeable_path);

        [DllImport("lockbook_core")]
        private static extern IntPtr create_account(IntPtr writeable_path, IntPtr username, IntPtr api_url);

        [DllImport("lockbook_core")]
        private static extern IntPtr import_account(IntPtr writeable_path, IntPtr account_string);

        [DllImport("lockbook_core")]
        private static extern IntPtr export_account(IntPtr writeable_path);

        [DllImport("lockbook_core")]
        private static extern IntPtr get_account(IntPtr writeable_path);

        [DllImport("lockbook_core")]
        private static extern IntPtr create_file_at_path(IntPtr writeable_path, IntPtr path_and_name);

        [DllImport("lockbook_core")]
        private static extern IntPtr write_document(IntPtr writeable_path, IntPtr id, IntPtr content);

        [DllImport("lockbook_core")]
        private static extern IntPtr create_file(IntPtr writeable_path, IntPtr name, IntPtr parent, IntPtr file_type);

        [DllImport("lockbook_core")]
        private static extern IntPtr get_root(IntPtr writeable_path);

        [DllImport("lockbook_core")]
        private static extern IntPtr get_children(IntPtr writeable_path, IntPtr id);

        [DllImport("lockbook_core")]
        private static extern IntPtr get_file_by_path(IntPtr writeable_path, IntPtr path);

        [DllImport("lockbook_core")]
        private static extern IntPtr read_document(IntPtr writeable_path, IntPtr id);

        [DllImport("lockbook_core")]
        private static extern IntPtr delete_file(IntPtr writeable_path, IntPtr id);

        [DllImport("lockbook_core")]
        private static extern IntPtr list_paths(IntPtr writeable_path, IntPtr filter);

        [DllImport("lockbook_core")]
        private static extern IntPtr rename_file(IntPtr writeable_path, IntPtr id, IntPtr new_name);

        [DllImport("lockbook_core")]
        private static extern IntPtr list_metadatas(IntPtr writeable_path);

        [DllImport("lockbook_core")]
        private static extern IntPtr move_file(IntPtr writeable_path, IntPtr id, IntPtr new_parent);

        [DllImport("lockbook_core")]
        private static extern IntPtr calculate_work(IntPtr writeable_path);

        [DllImport("lockbook_core")]
        private static extern IntPtr execute_work(IntPtr writeable_path, IntPtr work_unit);

        [DllImport("lockbook_core")]
        private static extern IntPtr sync_all(IntPtr writeable_path);

        [DllImport("lockbook_core")]
        private static extern IntPtr set_last_synced(IntPtr writeable_path, ulong last_sync);

        [DllImport("lockbook_core")]
        private static extern IntPtr get_last_synced(IntPtr writeable_path);

        [DllImport("lockbook_core")]
        private static extern IntPtr get_last_synced_human_string(IntPtr writeable_path);

        [DllImport("lockbook_core")]
        private static extern IntPtr get_usage(IntPtr writeable_path);

        [DllImport("lockbook_core")]
        private static extern IntPtr get_usage_human_string(IntPtr writeable_path, bool exact);

        private static string FromPtr(IntPtr pointer) {
            // Simplify to the commented code below when UWP supports .NET Standard 2.1
            // var result = Marshal.PtrToStringUTF8(pointer);
            // release_pointer(pointer);
            // return result;

            // https://stackoverflow.com/a/54745272
            int len = 0;
            while (Marshal.ReadByte(pointer, len) != 0) { ++len; } // read until null terminator
            byte[] buffer = new byte[len];
            Marshal.Copy(pointer, buffer, 0, buffer.Length);
            var result = Encoding.UTF8.GetString(buffer);
            release_pointer(pointer);
            return result;
        }

        // remember to Marshal.FreeHGlobal(yourPointer) when you're done!
        private static IntPtr ToPtr(string str) {
            var bytes = Encoding.UTF8.GetBytes(str).Concat(new byte[] { 0 }).ToArray(); // add null terminator
            var result = Marshal.AllocHGlobal(bytes.Length + 1);
            Marshal.Copy(bytes, 0, result, bytes.Length);
            return result;
        }

        public async Task<bool> AccountExists() {
            var getAccountIResult = await GetAccount();
            return getAccountIResult.GetType() == typeof(Core.GetAccount.Success);
        }

        private async Task<TIResult> FFICommon<TIResult, TExpectedErr, TPossibleErrs, TUnexpectedErr>(Func<IntPtr> func, Func<string, TIResult> parseOk)
            where TExpectedErr : ExpectedError<TPossibleErrs>, TIResult, new()
            where TPossibleErrs : struct, Enum
            where TUnexpectedErr : UnexpectedError, TIResult, new() {
            var result = await Task.Run(() => {
                string coreResponse;
                try {
                    coreMutex.WaitOne();
                    coreResponse = FromPtr(func());
                } finally {
                    coreMutex.ReleaseMutex();
                }
                return coreResponse;
            });

            var obj = JObject.Parse(result);
            var tag = obj.SelectToken("tag", errorWhenNoMatch: false)?.ToString();
            var content = obj.SelectToken("content", errorWhenNoMatch: false)?.ToString();
            if (tag == null) return UnexpectedErrors.New<TIResult, TUnexpectedErr>("contract error (no tag): " + result);
            if (content == null) return UnexpectedErrors.New<TIResult, TUnexpectedErr>("contract error (no content): " + result);
            switch (tag) {
                case "Ok":
                    return parseOk(content);
                case "Err":
                    var errObj = JObject.Parse(content);
                    var errTag = errObj.SelectToken("tag", errorWhenNoMatch: false)?.ToString();
                    var errContent = errObj.SelectToken("content", errorWhenNoMatch: false)?.ToString();
                    if (errTag == null) return UnexpectedErrors.New<TIResult, TUnexpectedErr>("contract error (no err tag): " + result);
                    if (errContent == null) return UnexpectedErrors.New<TIResult, TUnexpectedErr>("contract error (no err content): " + result);
                    switch (errTag) {
                        case "UiError":
                            if (Enum.TryParse<TPossibleErrs>(errContent, out var value)) return ExpectedErrors.New<TIResult, TExpectedErr, TPossibleErrs>(value);
                            return UnexpectedErrors.New<TIResult, TUnexpectedErr>("contract error (unknown UI err variant): " + result);
                        case "Unexpected":
                            return UnexpectedErrors.New<TIResult, TUnexpectedErr>(errContent);
                        default:
                            return UnexpectedErrors.New<TIResult, TUnexpectedErr>("contract error (err content tag neither UiError nor Unexpected): " + result);
                    }
                default:
                    return UnexpectedErrors.New<TIResult, TUnexpectedErr>("contract error (tag neither Ok nor Err): " + tag);
            }
        }

        public async Task InitLoggerSafely() {
            await Task.Run(() => {
                try {
                    coreMutex.WaitOne();
                    init_logger_safely(path);
                } finally {
                    coreMutex.ReleaseMutex();
                }
            });
        }

        public async Task<Core.GetDbState.IResult> GetDbState() {
            return await FFICommon<Core.GetDbState.IResult, Core.GetDbState.ExpectedError, Core.GetDbState.PossibleErrors, Core.GetDbState.UnexpectedError>(
                () => get_db_state(path),
                s => {
                    if (Enum.TryParse<DbState>(s, out var dbState)) {
                        return new Core.GetDbState.Success { dbState = dbState };
                    } else {
                        return new Core.GetDbState.UnexpectedError { ErrorMessage = "contract error (unknown dbState variant): " + s };
                    }
                });
        }

        public async Task<Core.MigrateDb.IResult> MigrateDb() {
            return await FFICommon<Core.MigrateDb.IResult, Core.MigrateDb.ExpectedError, Core.MigrateDb.PossibleErrors, Core.MigrateDb.UnexpectedError>(
                () => migrate_db(path),
                s => new Core.MigrateDb.Success());
        }

        public async Task<Core.CreateAccount.IResult> CreateAccount(string username, string apiUrl) {
            var usernamePtr = ToPtr(username);
            var apiUrlPtr = ToPtr(apiUrl);
            var result = await FFICommon<Core.CreateAccount.IResult, Core.CreateAccount.ExpectedError, Core.CreateAccount.PossibleErrors, Core.CreateAccount.UnexpectedError>(
                () => create_account(path, usernamePtr, apiUrlPtr),
                s => new Core.CreateAccount.Success());
            Marshal.FreeHGlobal(usernamePtr);
            Marshal.FreeHGlobal(apiUrlPtr);
            return result;
        }

        public async Task<Core.ImportAccount.IResult> ImportAccount(string accountString) {
            var accountStringPtr = ToPtr(accountString);
            var result = await FFICommon<Core.ImportAccount.IResult, Core.ImportAccount.ExpectedError, Core.ImportAccount.PossibleErrors, Core.ImportAccount.UnexpectedError>(
                () => import_account(path, accountStringPtr),
                s => new Core.ImportAccount.Success());
            Marshal.FreeHGlobal(accountStringPtr);
            return result;
        }

        public async Task<Core.ExportAccount.IResult> ExportAccount() {
            return await FFICommon<Core.ExportAccount.IResult, Core.ExportAccount.ExpectedError, Core.ExportAccount.PossibleErrors, Core.ExportAccount.UnexpectedError>(
                () => export_account(path),
                s => new Core.ExportAccount.Success { accountString = s });
        }

        public async Task<Core.GetAccount.IResult> GetAccount() {
            return await FFICommon<Core.GetAccount.IResult, Core.GetAccount.ExpectedError, Core.GetAccount.PossibleErrors, Core.GetAccount.UnexpectedError>(
                () => get_account(path),
                s => new Core.GetAccount.Success { account = JsonConvert.DeserializeObject<Account>(s) });
        }

        public async Task<Core.CreateFileAtPath.IResult> CreateFileAtPath(string pathWithName) {
            var pathWithNamePtr = ToPtr(pathWithName);
            var result = await FFICommon<Core.CreateFileAtPath.IResult, Core.CreateFileAtPath.ExpectedError, Core.CreateFileAtPath.PossibleErrors, Core.CreateFileAtPath.UnexpectedError>(
                () => create_file_at_path(path, pathWithNamePtr),
                s => new Core.CreateFileAtPath.Success { newFile = JsonConvert.DeserializeObject<FileMetadata>(s)});
            Marshal.FreeHGlobal(pathWithNamePtr);
            return result;
        }

        public async Task<Core.WriteDocument.IResult> WriteDocument(string id, string content) {
            var idPtr = ToPtr(id);
            var contentPtr = ToPtr(content);
            var result = await FFICommon<Core.WriteDocument.IResult, Core.WriteDocument.ExpectedError, Core.WriteDocument.PossibleErrors, Core.WriteDocument.UnexpectedError>(
                () => write_document(path, idPtr, contentPtr),
                s => new Core.WriteDocument.Success());
            Marshal.FreeHGlobal(idPtr);
            Marshal.FreeHGlobal(contentPtr);
            return result;
        }

        public async Task<Core.CreateFile.IResult> CreateFile(string name, string parent, FileType ft) {
            var namePtr = ToPtr(name);
            var parentPtr = ToPtr(parent);
            var fileTypePtr = ToPtr(ft == FileType.Folder ? "Folder" : "Document");
            var result = await FFICommon<Core.CreateFile.IResult, Core.CreateFile.ExpectedError, Core.CreateFile.PossibleErrors, Core.CreateFile.UnexpectedError>(
                () => create_file(path, namePtr, parentPtr, fileTypePtr),
                s => new Core.CreateFile.Success { newFile = JsonConvert.DeserializeObject<FileMetadata>(s) });
            Marshal.FreeHGlobal(namePtr);
            Marshal.FreeHGlobal(parentPtr);
            Marshal.FreeHGlobal(fileTypePtr);
            return result;
        }

        public async Task<Core.GetRoot.IResult> GetRoot() {
            return await FFICommon<Core.GetRoot.IResult, Core.GetRoot.ExpectedError, Core.GetRoot.PossibleErrors, Core.GetRoot.UnexpectedError>(
                () => get_root(path),
                s => new Core.GetRoot.Success { root = JsonConvert.DeserializeObject<FileMetadata>(s) });
        }

        public async Task<Core.GetChildren.IResult> GetChildren(string id) {
            var idPtr = ToPtr(id);
            var result = await FFICommon<Core.GetChildren.IResult, Core.GetChildren.ExpectedError, Core.GetChildren.PossibleErrors, Core.GetChildren.UnexpectedError>(
                () => get_children(path, idPtr),
                s => new Core.GetChildren.Success { children = JsonConvert.DeserializeObject<List<FileMetadata>>(s) });
            Marshal.FreeHGlobal(idPtr);
            return result;
        }

        public async Task<Core.ReadDocument.IResult> ReadDocument(string id) {
            var idPtr = ToPtr(id);
            var result = await FFICommon<Core.ReadDocument.IResult, Core.ReadDocument.ExpectedError, Core.ReadDocument.PossibleErrors, Core.ReadDocument.UnexpectedError>(
                () => read_document(path, idPtr),
                s => new Core.ReadDocument.Success { content = s });
            Marshal.FreeHGlobal(idPtr);
            return result;
        }

        public async Task<Core.GetFileByPath.IResult> GetFileByPath(string pathWithName) {
            var pathWithNamePtr = ToPtr(pathWithName);
            var result = await FFICommon<Core.GetFileByPath.IResult, Core.GetFileByPath.ExpectedError, Core.GetFileByPath.PossibleErrors, Core.GetFileByPath.UnexpectedError>(
                () => get_file_by_path(path, pathWithNamePtr),
                s => new Core.GetFileByPath.Success { file = JsonConvert.DeserializeObject<FileMetadata>(s) });
            Marshal.FreeHGlobal(pathWithNamePtr);
            return result;
        }

        public async Task<Core.DeleteFile.IResult> DeleteFile(string id) {
            var idPtr = ToPtr(id);
            var result = await FFICommon<Core.DeleteFile.IResult, Core.DeleteFile.ExpectedError, Core.DeleteFile.PossibleErrors, Core.DeleteFile.UnexpectedError>(
                () => delete_file(path, idPtr),
                s => new Core.DeleteFile.Success());
            Marshal.FreeHGlobal(idPtr);
            return result;
        }

        public async Task<Core.ListPaths.IResult> ListPaths(string filter) {
            var filterPtr = ToPtr(filter);
            var result = await FFICommon<Core.ListPaths.IResult, Core.ListPaths.ExpectedError, Core.ListPaths.PossibleErrors, Core.ListPaths.UnexpectedError>(
                () => list_paths(path, filterPtr),
                s => new Core.ListPaths.Success { paths = JsonConvert.DeserializeObject<List<string>>(s) });
            Marshal.FreeHGlobal(filterPtr);
            return result;
        }

        public async Task<Core.ListMetadatas.IResult> ListMetadatas() {
            return await FFICommon<Core.ListMetadatas.IResult, Core.ListMetadatas.ExpectedError, Core.ListMetadatas.PossibleErrors, Core.ListMetadatas.UnexpectedError>(
                () => list_metadatas(path),
                s => new Core.ListMetadatas.Success { files = JsonConvert.DeserializeObject<List<FileMetadata>>(s) });
        }

        public async Task<Core.RenameFile.IResult> RenameFile(string id, string newName) {
            var idPtr = ToPtr(id);
            var newNamePtr = ToPtr(newName);
            var result = await FFICommon<Core.RenameFile.IResult, Core.RenameFile.ExpectedError, Core.RenameFile.PossibleErrors, Core.RenameFile.UnexpectedError>(
                () => rename_file(path, idPtr, newNamePtr),
                s => new Core.RenameFile.Success());
            Marshal.FreeHGlobal(idPtr);
            Marshal.FreeHGlobal(newNamePtr);
            return result;
        }

        public async Task<Core.MoveFile.IResult> MoveFile(string id, string newParent) {
            var idPtr = ToPtr(id);
            var newParentPtr = ToPtr(newParent);
            var result = await FFICommon<Core.MoveFile.IResult, Core.MoveFile.ExpectedError, Core.MoveFile.PossibleErrors, Core.MoveFile.UnexpectedError>(
                () => move_file(path, idPtr, newParentPtr),
                s => new Core.MoveFile.Success());
            Marshal.FreeHGlobal(idPtr);
            Marshal.FreeHGlobal(newParentPtr);
            return result;
        }

        public async Task<Core.SyncAll.IResult> SyncAll() {
            return await FFICommon<Core.SyncAll.IResult, Core.SyncAll.ExpectedError, Core.SyncAll.PossibleErrors, Core.SyncAll.UnexpectedError>(
                () => sync_all(path),
                s => new Core.SyncAll.Success());
        }

        public async Task<Core.CalculateWork.IResult> CalculateWork() {
            return await FFICommon<Core.CalculateWork.IResult, Core.CalculateWork.ExpectedError, Core.CalculateWork.PossibleErrors, Core.CalculateWork.UnexpectedError>(
                () => calculate_work(path),
                s => new Core.CalculateWork.Success { workCalculated = JsonConvert.DeserializeObject<WorkCalculated>(s) });
        }

        public async Task<Core.ExecuteWork.IResult> ExecuteWork(string workUnit) {
            var workUnitPtr = ToPtr(workUnit);
            var result = await FFICommon<Core.ExecuteWork.IResult, Core.ExecuteWork.ExpectedError, Core.ExecuteWork.PossibleErrors, Core.ExecuteWork.UnexpectedError>(
                () => execute_work(path, workUnitPtr),
                s => new Core.ExecuteWork.Success());
            Marshal.FreeHGlobal(workUnitPtr);
            return result;
        }

        public async Task<Core.SetLastSynced.IResult> SetLastSynced(ulong lastSync) {
            return await FFICommon<Core.SetLastSynced.IResult, Core.SetLastSynced.ExpectedError, Core.SetLastSynced.PossibleErrors, Core.SetLastSynced.UnexpectedError>(
                () => set_last_synced(path, lastSync),
                s => new Core.SetLastSynced.Success());
        }

        public async Task<Core.GetLastSynced.IResult> GetLastSynced() {
            return await FFICommon<Core.GetLastSynced.IResult, Core.GetLastSynced.ExpectedError, Core.GetLastSynced.PossibleErrors, Core.GetLastSynced.UnexpectedError>(
                () => get_last_synced(path),
                s => new Core.GetLastSynced.Success { timestamp = JsonConvert.DeserializeObject<ulong>(s) });
        }

        public async Task<Core.GetLastSyncedHumanString.IResult> GetLastSyncedHumanString() {
            return await FFICommon<Core.GetLastSyncedHumanString.IResult, Core.GetLastSyncedHumanString.ExpectedError, Core.GetLastSyncedHumanString.PossibleErrors, Core.GetLastSyncedHumanString.UnexpectedError>(
                () => get_last_synced_human_string(path),
                s => new Core.GetLastSyncedHumanString.Success { timestamp = s });
        }

        public async Task<Core.GetUsage.IResult> GetUsage() {
            return await FFICommon<Core.GetUsage.IResult, Core.GetUsage.ExpectedError, Core.GetUsage.PossibleErrors, Core.GetUsage.UnexpectedError>(
                () => get_usage(path),
                s => new Core.GetUsage.Success { usage = JsonConvert.DeserializeObject<List<FileUsage>>(s) });
        }

        public async Task<Core.GetUsageHumanString.IResult> GetUsageHumanString() {
            return await FFICommon<Core.GetUsageHumanString.IResult, Core.GetUsageHumanString.ExpectedError, Core.GetUsageHumanString.PossibleErrors, Core.GetUsageHumanString.UnexpectedError>(
                () => get_usage_human_string(path, false),
                s => new Core.GetUsageHumanString.Success { usage = s });
        }
    }
}
