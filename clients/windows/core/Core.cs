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

        [DllImport("lockbook_core.dll")]
        private static extern void init_logger_safely(string writeable_path);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr create_account(string writeable_path, string username, string api_url);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr import_account(string writeable_path, string account_string);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr export_account(string writeable_path);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr get_account(string writeable_path);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr create_file_at_path(string writeable_path, string path_and_name);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr write_document(string writeable_path, string id, string content);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr create_file(string writeable_path, string name, string parent, string file_type);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr get_root(string writeable_path);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr get_file_by_path(string writeable_path, string path);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr read_document(string writeable_path, string id);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr list_paths(string writeable_path, string filter);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr rename_file(string writeable_path, string id, string new_name);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr list_metadatas(string writeable_path);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr move_file(string writeable_path, string id, string new_parent);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr calculate_work(string writeable_path);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr execute_work(string writeable_path, string work_unit);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr sync_all(string writeable_path);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr set_last_synced(string writeable_path, ulong last_synced);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr get_last_synced(string writeable_path);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr get_usage(string writeable_path);

        [DllImport("lockbook_core.dll")]
        private unsafe static extern void release_pointer(IntPtr str_pointer);

        private static string getStringAndRelease(IntPtr pointer) {
            string temp_string = Marshal.PtrToStringAnsi(pointer);
            string result = (string)temp_string.Clone();
            release_pointer(pointer);
            return result;
        }

        public async Task<bool> AccountExists() {
            var getAccountResult = await GetAccount();
            return getAccountResult.GetType() == typeof(Core.GetAccount.Success);
        }

        private async Task<T> FFICommon<T>(
            Func<IntPtr> func,
            Func<string, T> parseOk,
            Func<string, T> parseUIErr,
            Func<string, T> parseUnexpectedErr) {
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

            JObject obj = JObject.Parse(result);
            string tag = obj.SelectToken("tag", errorWhenNoMatch: false)?.ToString();
            string content = obj.SelectToken("content", errorWhenNoMatch: false)?.ToString();
            if (tag == null) return parseUnexpectedErr("contract error (no tag): " + result);
            if (content == null) return parseUnexpectedErr("contract error (no content): " + result);
            switch (tag) {
                case "Ok":
                    return parseOk(content);
                case "Err":
                    JObject errObj = JObject.Parse(content);
                    string errTag = errObj.SelectToken("tag", errorWhenNoMatch: false)?.ToString();
                    string errContent = errObj.SelectToken("content", errorWhenNoMatch: false)?.ToString();
                    if (errTag == null) return parseUnexpectedErr("contract error (no err tag): " + content);
                    if (errContent == null) return parseUnexpectedErr("contract error (no err content): " + content);
                    switch (errTag) {
                        case "UiError":
                            return parseUIErr(errContent);
                        case "Unexpected":
                            return parseUnexpectedErr(errContent);
                        default:
                            return parseUnexpectedErr("contract error (err content tag neither UiError nor Unexpected): " + content);
                    }
                default:
                    return parseUnexpectedErr("contract error (tag neither Ok nor Err): " + tag);
            }
        }

        public async Task<Core.CreateAccount.Result> CreateAccount(string username) {
            return await FFICommon<Core.CreateAccount.Result>(
                () => create_account(path, username, "http://qa.lockbook.app:8000"),
                s => {
                    return new Core.CreateAccount.Success();
                },
                s => {
                    if (new Dictionary<string, Core.CreateAccount.PossibleErrors> {
                        {"InvalidUsername", Core.CreateAccount.PossibleErrors.InvalidUsername },
                        {"UsernameTaken", Core.CreateAccount.PossibleErrors.UsernameTaken },
                        {"CouldNotReachServer", Core.CreateAccount.PossibleErrors.CouldNotReachServer },
                        {"AccountExistsAlready", Core.CreateAccount.PossibleErrors.AccountExistsAlready },
                    }.TryGetValue(s, out var p)) {
                        return new Core.CreateAccount.ExpectedError { error = p };
                    } else {
                        return new Core.CreateAccount.UnexpectedError { errorMessage = "contract error (unknown UIError): " + s };
                    }
                },
                s => {
                    return new Core.CreateAccount.UnexpectedError { errorMessage = s };
                });
        }

        public async Task<Core.GetAccount.Result> GetAccount() {
            return await FFICommon<Core.GetAccount.Result>(
                () => get_account(path),
                s => {
                    return new Core.GetAccount.Success();
                },
                s => {
                    if (new Dictionary<string, Core.GetAccount.PossibleErrors> {
                        {"NoAccount", Core.GetAccount.PossibleErrors.NoAccount },
                    }.TryGetValue(s, out var p)) {
                        return new Core.GetAccount.ExpectedError { error = p };
                    } else {
                        return new Core.GetAccount.UnexpectedError { errorMessage = "contract error (unknown UIError): " + s };
                    }
                },
                s => {
                    return new Core.GetAccount.UnexpectedError { errorMessage = s };
                });
        }

        public async Task<Core.ImportAccount.Result> ImportAccount(string account_string) {
            return await FFICommon<Core.ImportAccount.Result>(
                () => import_account(path, account_string),
                s => {
                    return new Core.ImportAccount.Success();
                },
                s => {
                    if (new Dictionary<string, Core.ImportAccount.PossibleErrors> {
                        {"AccountStringCorrupted", Core.ImportAccount.PossibleErrors.AccountStringCorrupted },
                        {"AccountExistsAlready", Core.ImportAccount.PossibleErrors.AccountExistsAlready },
                        {"AccountDoesNotExist", Core.ImportAccount.PossibleErrors.AccountDoesNotExist },
                        {"UsernamePKMismatch", Core.ImportAccount.PossibleErrors.UsernamePKMismatch },
                        {"CouldNotReachServer", Core.ImportAccount.PossibleErrors.CouldNotReachServer },
                    }.TryGetValue(s, out var p)) {
                        return new Core.ImportAccount.ExpectedError { error = p };
                    } else {
                        return new Core.ImportAccount.UnexpectedError { errorMessage = "contract error (unknown UIError): " + s };
                    }
                },
                s => {
                    return new Core.ImportAccount.UnexpectedError { errorMessage = s };
                });
        }

        public async Task<Core.ListMetadatas.Result> ListFileMetadata() {
            return await FFICommon<Core.ListMetadatas.Result>(
                () => list_metadatas(path),
                s => {
                    return new Core.ListMetadatas.Success();
                },
                s => {
                    return new Core.ListMetadatas.UnexpectedError { errorMessage = "contract error (unknown UIError): " + s };
                },
                s => {
                    return new Core.ListMetadatas.UnexpectedError { errorMessage = s };
                });
        }

        public async Task<Core.CreateFile.Result> CreateFile(string name, string parent, FileType ft) {
            return await FFICommon<Core.CreateFile.Result>(
                () => create_file(path, name, parent, ft == FileType.Folder ? "Folder" : "Document"),
                s => {
                    return new Core.CreateFile.Success();
                },
                s => {
                    if (new Dictionary<string, Core.CreateFile.PossibleErrors> {
                        {"NoAccount", Core.CreateFile.PossibleErrors.NoAccount },
                        {"DocumentTreatedAsFolder", Core.CreateFile.PossibleErrors.DocumentTreatedAsFolder },
                        {"CouldNotFindAParent", Core.CreateFile.PossibleErrors.CouldNotFindAParent },
                        {"FileNameNotAvailable", Core.CreateFile.PossibleErrors.FileNameNotAvailable },
                        {"FileNameContainsSlash", Core.CreateFile.PossibleErrors.FileNameContainsSlash },
                    }.TryGetValue(s, out var p)) {
                        return new Core.CreateFile.ExpectedError { error = p };
                    } else {
                        return new Core.CreateFile.UnexpectedError { errorMessage = "contract error (unknown UIError): " + s };
                    }
                },
                s => {
                    return new Core.CreateFile.UnexpectedError { errorMessage = s };
                });
        }

        public async Task<Core.SyncAll.Result> SyncAll() {
            return await FFICommon<Core.SyncAll.Result>(
                () => sync_all(path),
                s => {
                    return new Core.SyncAll.Success();
                },
                s => {
                    if (new Dictionary<string, Core.SyncAll.PossibleErrors> {
                        {"NoAccount", Core.SyncAll.PossibleErrors.NoAccount },
                        {"CouldNotReachServer", Core.SyncAll.PossibleErrors.CouldNotReachServer },
                        {"ExecuteWorkError", Core.SyncAll.PossibleErrors.ExecuteWorkError },
                    }.TryGetValue(s, out var p)) {
                        return new Core.SyncAll.ExpectedError { error = p };
                    } else {
                        return new Core.SyncAll.UnexpectedError { errorMessage = "contract error (unknown UIError): " + s };
                    }
                },
                s => {
                    return new Core.SyncAll.UnexpectedError { errorMessage = s };
                });
        }

        public async Task<Core.RenameFile.Result> RenameFile(string id, string newName) {
            return await FFICommon<Core.RenameFile.Result>(
                () => rename_file(path, id, newName),
                s => {
                    return new Core.RenameFile.Success();
                },
                s => {
                    if (new Dictionary<string, Core.RenameFile.PossibleErrors> {
                        {"FileDoesNotExist", Core.RenameFile.PossibleErrors.FileDoesNotExist },
                        {"NewNameContainsSlash", Core.RenameFile.PossibleErrors.NewNameContainsSlash },
                        {"FileNameNotAvailable", Core.RenameFile.PossibleErrors.FileNameNotAvailable },
                    }.TryGetValue(s, out var p)) {
                        return new Core.RenameFile.ExpectedError { error = p };
                    } else {
                        return new Core.RenameFile.UnexpectedError { errorMessage = "contract error (unknown UIError): " + s };
                    }
                },
                s => {
                    return new Core.RenameFile.UnexpectedError { errorMessage = s };
                });
        }

        public async Task<Core.MoveFile.Result> MoveFile(string id, string newParent) {
            return await FFICommon<Core.MoveFile.Result>(
                () => move_file(path, id, newParent),
                s => {
                    return new Core.MoveFile.Success();
                },
                s => {
                    if (new Dictionary<string, Core.MoveFile.PossibleErrors> {
                        {"NoAccount", Core.MoveFile.PossibleErrors.NoAccount },
                        {"FileDoesNotExist", Core.MoveFile.PossibleErrors.FileDoesNotExist },
                        {"DocumentTreatedAsFolder", Core.MoveFile.PossibleErrors.DocumentTreatedAsFolder },
                        {"TargetParentHasChildNamedThat", Core.MoveFile.PossibleErrors.TargetParentHasChildNamedThat },
                        {"TargetParentDoesNotExist", Core.MoveFile.PossibleErrors.TargetParentDoesNotExist },
                    }.TryGetValue(s, out var p)) {
                        return new Core.MoveFile.ExpectedError { error = p };
                    } else {
                        return new Core.MoveFile.UnexpectedError { errorMessage = "contract error (unknown UIError): " + s };
                    }
                },
                s => {
                    return new Core.MoveFile.UnexpectedError { errorMessage = s };
                });
        }

        public async Task<Core.ReadDocument.Result> ReadDocument(string id) {
            return await FFICommon<Core.ReadDocument.Result>(
                () => read_document(path, id),
                s => {
                    return new Core.ReadDocument.Success();
                },
                s => {
                    if (new Dictionary<string, Core.ReadDocument.PossibleErrors> {
                        {"NoAccount", Core.ReadDocument.PossibleErrors.NoAccount },
                        {"FileDoesNotExist", Core.ReadDocument.PossibleErrors.FileDoesNotExist },
                        {"TreatedFolderAsDocument", Core.ReadDocument.PossibleErrors.TreatedFolderAsDocument },
                    }.TryGetValue(s, out var p)) {
                        return new Core.ReadDocument.ExpectedError { error = p };
                    } else {
                        return new Core.ReadDocument.UnexpectedError { errorMessage = "contract error (unknown UIError): " + s };
                    }
                },
                s => {
                    return new Core.ReadDocument.UnexpectedError { errorMessage = s };
                });
        }

        public async Task<Core.WriteDocument.Result> WriteDocument(string id, string content) {
            return await FFICommon<Core.WriteDocument.Result>(
                () => write_document(path, id, content),
                s => {
                    return new Core.WriteDocument.Success();
                },
                s => {
                    if (new Dictionary<string, Core.WriteDocument.PossibleErrors> {
                        {"NoAccount", Core.WriteDocument.PossibleErrors.NoAccount },
                        {"FileDoesNotExist", Core.WriteDocument.PossibleErrors.FileDoesNotExist },
                        {"TreatedFolderAsDocument", Core.WriteDocument.PossibleErrors.TreatedFolderAsDocument },
                    }.TryGetValue(s, out var p)) {
                        return new Core.WriteDocument.ExpectedError { error = p };
                    } else {
                        return new Core.WriteDocument.UnexpectedError { errorMessage = "contract error (unknown UIError): " + s };
                    }
                },
                s => {
                    return new Core.WriteDocument.UnexpectedError { errorMessage = s };
                });
        }

        public async Task<Core.CalculateWork.Result> CalculateWork() {
            return await FFICommon<Core.CalculateWork.Result>(
                () => calculate_work(path),
                s => {
                    return new Core.CalculateWork.Success();
                },
                s => {
                    if (new Dictionary<string, Core.CalculateWork.PossibleErrors> {
                        {"NoAccount", Core.CalculateWork.PossibleErrors.NoAccount },
                        {"CouldNotReachServer", Core.CalculateWork.PossibleErrors.CouldNotReachServer },
                    }.TryGetValue(s, out var p)) {
                        return new Core.CalculateWork.ExpectedError { error = p };
                    } else {
                        return new Core.CalculateWork.UnexpectedError { errorMessage = "contract error (unknown UIError): " + s };
                    }
                },
                s => {
                    return new Core.CalculateWork.UnexpectedError { errorMessage = s };
                });
        }
    }
}
