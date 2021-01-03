using Core;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
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
        public string path;

        public CoreService(string path) {
            this.path = path;
        }

        private static Mutex coreMutex = new Mutex();

        [DllImport("lockbook_core")]
        private unsafe static extern void release_pointer(IntPtr str_pointer);

        [DllImport("lockbook_core")]
        private static extern void init_logger_safely(string writeable_path);

        [DllImport("lockbook_core")]
        private static extern IntPtr get_db_state(string writeable_path);

        [DllImport("lockbook_core")]
        private static extern IntPtr migrate_db(string writeable_path);

        [DllImport("lockbook_core")]
        private static extern IntPtr create_account(string writeable_path, string username, string api_url);

        [DllImport("lockbook_core")]
        private static extern IntPtr import_account(string writeable_path, string account_string);

        [DllImport("lockbook_core")]
        private static extern IntPtr export_account(string writeable_path);

        [DllImport("lockbook_core")]
        private static extern IntPtr get_account(string writeable_path);

        [DllImport("lockbook_core")]
        private static extern IntPtr create_file_at_path(string writeable_path, string path_and_name);

        [DllImport("lockbook_core")]
        private static extern IntPtr write_document(string writeable_path, string id, string content);

        [DllImport("lockbook_core")]
        private static extern IntPtr create_file(string writeable_path, string name, string parent, string file_type);

        [DllImport("lockbook_core")]
        private static extern IntPtr get_root(string writeable_path);

        [DllImport("lockbook_core")]
        private static extern IntPtr get_children(string writeable_path, string id);

        [DllImport("lockbook_core")]
        private static extern IntPtr get_file_by_path(string writeable_path, string path);

        [DllImport("lockbook_core")]
        private static extern IntPtr read_document(string writeable_path, string id);

        [DllImport("lockbook_core")]
        private static extern IntPtr delete_file(string writeable_path, string id);

        [DllImport("lockbook_core")]
        private static extern IntPtr list_paths(string writeable_path, string filter);

        [DllImport("lockbook_core")]
        private static extern IntPtr rename_file(string writeable_path, string id, string new_name);

        [DllImport("lockbook_core")]
        private static extern IntPtr list_metadatas(string writeable_path);

        [DllImport("lockbook_core")]
        private static extern IntPtr move_file(string writeable_path, string id, string new_parent);

        [DllImport("lockbook_core")]
        private static extern IntPtr calculate_work(string writeable_path);

        [DllImport("lockbook_core")]
        private static extern IntPtr execute_work(string writeable_path, string work_unit);

        [DllImport("lockbook_core")]
        private static extern IntPtr sync_all(string writeable_path);

        [DllImport("lockbook_core")]
        private static extern IntPtr set_last_synced(string writeable_path, ulong last_sync);

        [DllImport("lockbook_core")]
        private static extern IntPtr get_last_synced(string writeable_path);

        [DllImport("lockbook_core")]
        private static extern IntPtr get_usage(string writeable_path);

        private static string getStringAndRelease(IntPtr pointer) {
            string temp_string = Marshal.PtrToStringAnsi(pointer);
            string IResult = (string)temp_string.Clone();
            release_pointer(pointer);
            return IResult;
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
                    coreResponse = getStringAndRelease(func());
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
            return await FFICommon<Core.CreateAccount.IResult, Core.CreateAccount.ExpectedError, Core.CreateAccount.PossibleErrors, Core.CreateAccount.UnexpectedError>(
                () => create_account(path, username, apiUrl),
                s => new Core.CreateAccount.Success());
        }

        public async Task<Core.ImportAccount.IResult> ImportAccount(string accountString) {
            return await FFICommon<Core.ImportAccount.IResult, Core.ImportAccount.ExpectedError, Core.ImportAccount.PossibleErrors, Core.ImportAccount.UnexpectedError>(
                () => import_account(path, accountString),
                s => new Core.ImportAccount.Success());
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
            return await FFICommon<Core.CreateFileAtPath.IResult, Core.CreateFileAtPath.ExpectedError, Core.CreateFileAtPath.PossibleErrors, Core.CreateFileAtPath.UnexpectedError>(
                () => create_file_at_path(path, pathWithName),
                s => new Core.CreateFileAtPath.Success { newFile = JsonConvert.DeserializeObject<FileMetadata>(s)});
        }

        public async Task<Core.WriteDocument.IResult> WriteDocument(string id, string content) {
            return await FFICommon<Core.WriteDocument.IResult, Core.WriteDocument.ExpectedError, Core.WriteDocument.PossibleErrors, Core.WriteDocument.UnexpectedError>(
                () => write_document(path, id, content),
                s => new Core.WriteDocument.Success());
        }

        public async Task<Core.CreateFile.IResult> CreateFile(string name, string parent, FileType ft) {
            return await FFICommon<Core.CreateFile.IResult, Core.CreateFile.ExpectedError, Core.CreateFile.PossibleErrors, Core.CreateFile.UnexpectedError>(
                () => create_file(path, name, parent, ft == FileType.Folder ? "Folder" : "Document"),
                s => new Core.CreateFile.Success { newFile = JsonConvert.DeserializeObject<FileMetadata>(s) });
        }

        public async Task<Core.GetRoot.IResult> GetRoot() {
            return await FFICommon<Core.GetRoot.IResult, Core.GetRoot.ExpectedError, Core.GetRoot.PossibleErrors, Core.GetRoot.UnexpectedError>(
                () => get_root(path),
                s => new Core.GetRoot.Success { root = JsonConvert.DeserializeObject<FileMetadata>(s) });
        }

        public async Task<Core.GetChildren.IResult> GetChildren(string id) {
            return await FFICommon<Core.GetChildren.IResult, Core.GetChildren.ExpectedError, Core.GetChildren.PossibleErrors, Core.GetChildren.UnexpectedError>(
                () => get_children(path, id),
                s => new Core.GetChildren.Success { children = JsonConvert.DeserializeObject<List<FileMetadata>>(s) });
        }

        public async Task<Core.ReadDocument.IResult> ReadDocument(string id) {
            return await FFICommon<Core.ReadDocument.IResult, Core.ReadDocument.ExpectedError, Core.ReadDocument.PossibleErrors, Core.ReadDocument.UnexpectedError>(
                () => read_document(path, id),
                s => new Core.ReadDocument.Success { content = s });
        }

        public async Task<Core.GetFileByPath.IResult> GetFileByPath(string pathWithName) {
            return await FFICommon<Core.GetFileByPath.IResult, Core.GetFileByPath.ExpectedError, Core.GetFileByPath.PossibleErrors, Core.GetFileByPath.UnexpectedError>(
                () => get_file_by_path(path, pathWithName),
                s => new Core.GetFileByPath.Success { file = JsonConvert.DeserializeObject<FileMetadata>(s) });
        }

        public async Task<Core.DeleteFile.IResult> DeleteFile(string id) {
            return await FFICommon<Core.DeleteFile.IResult, Core.DeleteFile.ExpectedError, Core.DeleteFile.PossibleErrors, Core.DeleteFile.UnexpectedError>(
                () => delete_file(path, id),
                s => new Core.DeleteFile.Success());
        }

        public async Task<Core.ListPaths.IResult> ListPaths(string filter) {
            return await FFICommon<Core.ListPaths.IResult, Core.ListPaths.ExpectedError, Core.ListPaths.PossibleErrors, Core.ListPaths.UnexpectedError>(
                () => list_paths(path, filter),
                s => new Core.ListPaths.Success { paths = JsonConvert.DeserializeObject<List<string>>(s) });
        }

        public async Task<Core.ListMetadatas.IResult> ListMetadatas() {
            return await FFICommon<Core.ListMetadatas.IResult, Core.ListMetadatas.ExpectedError, Core.ListMetadatas.PossibleErrors, Core.ListMetadatas.UnexpectedError>(
                () => list_metadatas(path),
                s => new Core.ListMetadatas.Success { files = JsonConvert.DeserializeObject<List<FileMetadata>>(s) });
        }

        public async Task<Core.RenameFile.IResult> RenameFile(string id, string newName) {
            return await FFICommon<Core.RenameFile.IResult, Core.RenameFile.ExpectedError, Core.RenameFile.PossibleErrors, Core.RenameFile.UnexpectedError>(
                () => rename_file(path, id, newName),
                s => new Core.RenameFile.Success());
        }

        public async Task<Core.MoveFile.IResult> MoveFile(string id, string newParent) {
            return await FFICommon<Core.MoveFile.IResult, Core.MoveFile.ExpectedError, Core.MoveFile.PossibleErrors, Core.MoveFile.UnexpectedError>(
                () => move_file(path, id, newParent),
                s => new Core.MoveFile.Success());
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
            return await FFICommon<Core.ExecuteWork.IResult, Core.ExecuteWork.ExpectedError, Core.ExecuteWork.PossibleErrors, Core.ExecuteWork.UnexpectedError>(
                () => execute_work(path, workUnit),
                s => new Core.ExecuteWork.Success());
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

        public async Task<Core.GetUsage.IResult> GetUsage() {
            return await FFICommon<Core.GetUsage.IResult, Core.GetUsage.ExpectedError, Core.GetUsage.PossibleErrors, Core.GetUsage.UnexpectedError>(
                () => get_usage(path),
                s => new Core.GetUsage.Success { usage = JsonConvert.DeserializeObject<List<FileUsage>>(s) });
        }
    }
}
