using core;
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
        public IntPtr path;
        private JsonSerializerSettings jsonSettings = new JsonSerializerSettings { MissingMemberHandling = MissingMemberHandling.Error };

        public CoreService(string path) {
            this.path = Utils.ToFFI(path);
        }

        private static Mutex coreMutex = new Mutex();

        [DllImport("lockbook_core", ExactSpelling = true)]
        private unsafe static extern void release_pointer(IntPtr str_pointer);

        [DllImport("lockbook_core", ExactSpelling = true)]
        private static extern IntPtr init(IntPtr writeable_path, bool logs);

        [DllImport("lockbook_core", ExactSpelling = true)]
        private static extern IntPtr create_account(IntPtr username, IntPtr api_url);

        [DllImport("lockbook_core", ExactSpelling = true)]
        private static extern IntPtr import_account(IntPtr account_string);

        [DllImport("lockbook_core", ExactSpelling = true)]
        private static extern IntPtr export_account();

        [DllImport("lockbook_core", ExactSpelling = true)]
        private static extern IntPtr get_account();

        [DllImport("lockbook_core", ExactSpelling = true)]
        private static extern IntPtr create_file_at_path(IntPtr path_and_name);

        [DllImport("lockbook_core", ExactSpelling = true)]
        private static extern IntPtr write_document(IntPtr id, IntPtr content);

        [DllImport("lockbook_core", ExactSpelling = true)]
        private static extern IntPtr create_file(IntPtr name, IntPtr parent, IntPtr file_type);

        [DllImport("lockbook_core", ExactSpelling = true)]
        private static extern IntPtr get_root();

        [DllImport("lockbook_core", ExactSpelling = true)]
        private static extern IntPtr get_children(IntPtr id);

        [DllImport("lockbook_core", ExactSpelling = true)]
        private static extern IntPtr get_by_path(IntPtr path);

        [DllImport("lockbook_core", ExactSpelling = true)]
        private static extern IntPtr read_document(IntPtr id);

        [DllImport("lockbook_core", ExactSpelling = true)]
        private static extern IntPtr delete_file(IntPtr id);

        [DllImport("lockbook_core", ExactSpelling = true)]
        private static extern IntPtr list_paths(IntPtr filter);

        [DllImport("lockbook_core", ExactSpelling = true)]
        private static extern IntPtr rename_file(IntPtr id, IntPtr new_name);

        [DllImport("lockbook_core", ExactSpelling = true)]
        private static extern IntPtr list_metadatas();

        [DllImport("lockbook_core", ExactSpelling = true)]
        private static extern IntPtr move_file(IntPtr id, IntPtr new_parent);

        [DllImport("lockbook_core", ExactSpelling = true)]
        private static extern IntPtr calculate_work();

        [DllImport("lockbook_core", ExactSpelling = true)]
        private static extern IntPtr sync_all();

        [DllImport("lockbook_core", ExactSpelling = true)]
        private static extern IntPtr get_last_synced();

        [DllImport("lockbook_core", ExactSpelling = true)]
        private static extern IntPtr get_last_synced_human_string();

        [DllImport("lockbook_core", ExactSpelling = true)]
        private static extern IntPtr get_usage();

        [DllImport("lockbook_core", ExactSpelling = true)]
        private static extern IntPtr get_variants();

        private static string CopyToManagedAndRelease(IntPtr ptr) {
            var result = Utils.FromFFI(ptr);
            release_pointer(ptr);
            return result;
        }

        public async Task<bool> AccountExists() {
            var getAccountIResult = await GetAccount();
            return getAccountIResult.GetType() == typeof(Core.GetAccount.Success);
        }

        private async Task<string> RunAsyncWithMutex(Func<IntPtr> func) {
            return await Task.Run(() => {
                string coreResponse;
                try {
                    coreMutex.WaitOne();
                    coreResponse = CopyToManagedAndRelease(func());
                } finally {
                    coreMutex.ReleaseMutex();
                }
                return coreResponse;
            });
        }

        private async Task<TIResult> FFICommon<TIResult, TExpectedErr, TPossibleErrs, TUnexpectedErr>(Func<IntPtr> func, Func<string, TIResult> parseOk)
            where TExpectedErr : ExpectedError<TPossibleErrs>, TIResult, new()
            where TPossibleErrs : struct, Enum
            where TUnexpectedErr : UnexpectedError, TIResult, new() {
            var result = await RunAsyncWithMutex(func);

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

        public async Task Init() {
            await Task.Run(() => {
                try {
                    coreMutex.WaitOne();
                    init(path, true);
                } finally {
                    coreMutex.ReleaseMutex();
                }
            });
        }

        public async Task<Core.CreateAccount.IResult> CreateAccount(string username, string apiUrl) {
            var usernamePtr = Utils.ToFFI(username);
            var apiUrlPtr = Utils.ToFFI(apiUrl);
            var result = await FFICommon<Core.CreateAccount.IResult, Core.CreateAccount.ExpectedError, Core.CreateAccount.PossibleErrors, Core.CreateAccount.UnexpectedError>(
                () => create_account(usernamePtr, apiUrlPtr),
                s => new Core.CreateAccount.Success());
            Marshal.FreeHGlobal(usernamePtr);
            Marshal.FreeHGlobal(apiUrlPtr);
            return result;
        }

        public async Task<Core.ImportAccount.IResult> ImportAccount(string accountString) {
            var accountStringPtr = Utils.ToFFI(accountString);
            var result = await FFICommon<Core.ImportAccount.IResult, Core.ImportAccount.ExpectedError, Core.ImportAccount.PossibleErrors, Core.ImportAccount.UnexpectedError>(
                () => import_account(accountStringPtr),
                s => new Core.ImportAccount.Success());
            Marshal.FreeHGlobal(accountStringPtr);
            return result;
        }

        public async Task<Core.ExportAccount.IResult> ExportAccount() {
            return await FFICommon<Core.ExportAccount.IResult, Core.ExportAccount.ExpectedError, Core.ExportAccount.PossibleErrors, Core.ExportAccount.UnexpectedError>(
                () => export_account(),
                s => new Core.ExportAccount.Success { accountString = s });
        }

        public async Task<Core.GetAccount.IResult> GetAccount() {
            return await FFICommon<Core.GetAccount.IResult, Core.GetAccount.ExpectedError, Core.GetAccount.PossibleErrors, Core.GetAccount.UnexpectedError>(
                () => get_account(),
                s => new Core.GetAccount.Success { account = JsonConvert.DeserializeObject<Account>(s, jsonSettings) });
        }

        public async Task<Core.CreateFileAtPath.IResult> CreateFileAtPath(string pathWithName) {
            var pathWithNamePtr = Utils.ToFFI(pathWithName);
            var result = await FFICommon<Core.CreateFileAtPath.IResult, Core.CreateFileAtPath.ExpectedError, Core.CreateFileAtPath.PossibleErrors, Core.CreateFileAtPath.UnexpectedError>(
                () => create_file_at_path(pathWithNamePtr),
                s => new Core.CreateFileAtPath.Success { newFile = JsonConvert.DeserializeObject<DecryptedFileMetadata>(s, jsonSettings) });
            Marshal.FreeHGlobal(pathWithNamePtr);
            return result;
        }

        public async Task<Core.WriteDocument.IResult> WriteDocument(string id, string content) {
            var idPtr = Utils.ToFFI(id);
            var contentPtr = Utils.ToFFI(content);
            var result = await FFICommon<Core.WriteDocument.IResult, Core.WriteDocument.ExpectedError, Core.WriteDocument.PossibleErrors, Core.WriteDocument.UnexpectedError>(
                () => write_document(idPtr, contentPtr),
                s => new Core.WriteDocument.Success());
            Marshal.FreeHGlobal(idPtr);
            Marshal.FreeHGlobal(contentPtr);
            return result;
        }

        public async Task<Core.CreateFile.IResult> CreateFile(string name, string parent, FileType ft) {
            var namePtr = Utils.ToFFI(name);
            var parentPtr = Utils.ToFFI(parent);
            var fileTypePtr = Utils.ToFFI(ft == FileType.Folder ? "Folder" : "Document");
            var result = await FFICommon<Core.CreateFile.IResult, Core.CreateFile.ExpectedError, Core.CreateFile.PossibleErrors, Core.CreateFile.UnexpectedError>(
                () => create_file(namePtr, parentPtr, fileTypePtr),
                s => new Core.CreateFile.Success { newFile = JsonConvert.DeserializeObject<DecryptedFileMetadata>(s, jsonSettings) });
            Marshal.FreeHGlobal(namePtr);
            Marshal.FreeHGlobal(parentPtr);
            Marshal.FreeHGlobal(fileTypePtr);
            return result;
        }

        public async Task<Core.GetRoot.IResult> GetRoot() {
            return await FFICommon<Core.GetRoot.IResult, Core.GetRoot.ExpectedError, Core.GetRoot.PossibleErrors, Core.GetRoot.UnexpectedError>(
                () => get_root(),
                s => new Core.GetRoot.Success { root = JsonConvert.DeserializeObject<DecryptedFileMetadata>(s, jsonSettings) });
        }

        public async Task<Core.GetChildren.IResult> GetChildren(string id) {
            var idPtr = Utils.ToFFI(id);
            var result = await FFICommon<Core.GetChildren.IResult, Core.GetChildren.ExpectedError, Core.GetChildren.PossibleErrors, Core.GetChildren.UnexpectedError>(
                () => get_children(idPtr),
                s => new Core.GetChildren.Success { children = JsonConvert.DeserializeObject<List<DecryptedFileMetadata>>(s, jsonSettings) });
            Marshal.FreeHGlobal(idPtr);
            return result;
        }

        public async Task<Core.ReadDocument.IResult> ReadDocument(string id) {
            var idPtr = Utils.ToFFI(id);
            var result = await FFICommon<Core.ReadDocument.IResult, Core.ReadDocument.ExpectedError, Core.ReadDocument.PossibleErrors, Core.ReadDocument.UnexpectedError>(
                () => read_document(idPtr),
                s => new Core.ReadDocument.Success { content = s });
            Marshal.FreeHGlobal(idPtr);
            return result;
        }

        public async Task<Core.GetByPath.IResult> GetByPath(string pathWithName) {
            var pathWithNamePtr = Utils.ToFFI(pathWithName);
            var result = await FFICommon<Core.GetByPath.IResult, Core.GetByPath.ExpectedError, Core.GetByPath.PossibleErrors, Core.GetByPath.UnexpectedError>(
                () => get_by_path(pathWithNamePtr),
                s => new Core.GetByPath.Success { file = JsonConvert.DeserializeObject<DecryptedFileMetadata>(s, jsonSettings) });
            Marshal.FreeHGlobal(pathWithNamePtr);
            return result;
        }

        public async Task<Core.DeleteFile.IResult> DeleteFile(string id) {
            var idPtr = Utils.ToFFI(id);
            var result = await FFICommon<Core.DeleteFile.IResult, Core.DeleteFile.ExpectedError, Core.DeleteFile.PossibleErrors, Core.DeleteFile.UnexpectedError>(
                () => delete_file(idPtr),
                s => new Core.DeleteFile.Success());
            Marshal.FreeHGlobal(idPtr);
            return result;
        }

        public async Task<Core.ListPaths.IResult> ListPaths(string filter) {
            var filterPtr = Utils.ToFFI(filter);
            var result = await FFICommon<Core.ListPaths.IResult, Core.ListPaths.ExpectedError, Core.ListPaths.PossibleErrors, Core.ListPaths.UnexpectedError>(
                () => list_paths(filterPtr),
                s => new Core.ListPaths.Success { paths = JsonConvert.DeserializeObject<List<string>>(s, jsonSettings) });
            Marshal.FreeHGlobal(filterPtr);
            return result;
        }

        public async Task<Core.ListMetadatas.IResult> ListMetadatas() {
            return await FFICommon<Core.ListMetadatas.IResult, Core.ListMetadatas.ExpectedError, Core.ListMetadatas.PossibleErrors, Core.ListMetadatas.UnexpectedError>(
                () => list_metadatas(),
                s => new Core.ListMetadatas.Success { files = JsonConvert.DeserializeObject<List<DecryptedFileMetadata>>(s, jsonSettings) });
        }

        public async Task<Core.RenameFile.IResult> RenameFile(string id, string newName) {
            var idPtr = Utils.ToFFI(id);
            var newNamePtr = Utils.ToFFI(newName);
            var result = await FFICommon<Core.RenameFile.IResult, Core.RenameFile.ExpectedError, Core.RenameFile.PossibleErrors, Core.RenameFile.UnexpectedError>(
                () => rename_file(idPtr, newNamePtr),
                s => new Core.RenameFile.Success());
            Marshal.FreeHGlobal(idPtr);
            Marshal.FreeHGlobal(newNamePtr);
            return result;
        }

        public async Task<Core.MoveFile.IResult> MoveFile(string id, string newParent) {
            var idPtr = Utils.ToFFI(id);
            var newParentPtr = Utils.ToFFI(newParent);
            var result = await FFICommon<Core.MoveFile.IResult, Core.MoveFile.ExpectedError, Core.MoveFile.PossibleErrors, Core.MoveFile.UnexpectedError>(
                () => move_file(idPtr, newParentPtr),
                s => new Core.MoveFile.Success());
            Marshal.FreeHGlobal(idPtr);
            Marshal.FreeHGlobal(newParentPtr);
            return result;
        }

        public async Task<Core.SyncAll.IResult> SyncAll() {
            return await FFICommon<Core.SyncAll.IResult, Core.SyncAll.ExpectedError, Core.SyncAll.PossibleErrors, Core.SyncAll.UnexpectedError>(
                () => sync_all(),
                s => new Core.SyncAll.Success());
        }

        public async Task<Core.CalculateWork.IResult> CalculateWork() {
            return await FFICommon<Core.CalculateWork.IResult, Core.CalculateWork.ExpectedError, Core.CalculateWork.PossibleErrors, Core.CalculateWork.UnexpectedError>(
                () => calculate_work(),
                s => new Core.CalculateWork.Success { workCalculated = JsonConvert.DeserializeObject<WorkCalculated>(s, jsonSettings) });
        }

        public async Task<Core.GetLastSynced.IResult> GetLastSynced() {
            return await FFICommon<Core.GetLastSynced.IResult, Core.GetLastSynced.ExpectedError, Core.GetLastSynced.PossibleErrors, Core.GetLastSynced.UnexpectedError>(
                () => get_last_synced(),
                s => new Core.GetLastSynced.Success { timestamp = JsonConvert.DeserializeObject<ulong>(s, jsonSettings) });
        }

        public async Task<Core.GetLastSyncedHumanString.IResult> GetLastSyncedHumanString() {
            return await FFICommon<Core.GetLastSyncedHumanString.IResult, Core.GetLastSyncedHumanString.ExpectedError, Core.GetLastSyncedHumanString.PossibleErrors, Core.GetLastSyncedHumanString.UnexpectedError>(
                () => get_last_synced_human_string(),
                s => new Core.GetLastSyncedHumanString.Success { timestamp = s });
        }

        public async Task<Core.GetUsage.IResult> GetUsage() {
            return await FFICommon<Core.GetUsage.IResult, Core.GetUsage.ExpectedError, Core.GetUsage.PossibleErrors, Core.GetUsage.UnexpectedError>(
                () => get_usage(),
                s => new Core.GetUsage.Success { usage = JsonConvert.DeserializeObject<UsageMetrics>(s, jsonSettings) });
        }

        public async Task<Dictionary<string, List<string>>> GetVariants() {
            var result = await RunAsyncWithMutex(get_variants);
            return JsonConvert.DeserializeObject<Dictionary<string, List<string>>>(result, jsonSettings);
        }
    }
}
