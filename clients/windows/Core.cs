using Core;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Threading;
using System.Threading.Tasks;
using Windows.Data.Json;

namespace lockbook {
    class CoreService {
        static String path = Windows.Storage.ApplicationData.Current.LocalFolder.Path;
        private static Mutex coreMutex = new Mutex();


        [DllImport("lockbook_core.dll")]
        private static extern IntPtr get_api_loc();

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr create_account(string path, string username);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr get_account(string path);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr import_account(string path, string account_string);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr list_metadatas(string path);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr create_file(string path, string name, string parent, string file_type);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr rename_file(string path, string id, string new_name);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr move_file(string path, string id, string new_parent);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr sync_all(string path);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr read_document(string path, string id);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr write_document(string path, string id, string content);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr calculate_work(string path);

        [DllImport("lockbook_core.dll")]
        private unsafe static extern void release_pointer(IntPtr str_pointer);



        private static String getStringAndRelease(IntPtr pointer) {
            String temp_string = Marshal.PtrToStringAnsi(pointer);
            String result = (String)temp_string.Clone();
            release_pointer(pointer);
            return result;
        }

        public static bool AccountExists() {
            coreMutex.WaitOne();
            String result = getStringAndRelease(get_account(path));
            coreMutex.ReleaseMutex();
            JObject obj = JObject.Parse(result);
            JToken ok = obj.SelectToken("Ok", errorWhenNoMatch: false);
            return ok != null;
        }

        public static async Task<Core.CreateAccount.Result> CreateAccount(String username) {
            String result = await Task.Run(() => {
                coreMutex.WaitOne();
                String coreResponse = getStringAndRelease(create_account(path, username));
                coreMutex.ReleaseMutex();
                return coreResponse;
            });

            JObject obj = JObject.Parse(result);

            JToken unexpectedError = obj.SelectToken("Err.UnexpectedError", errorWhenNoMatch: false);
            JToken expectedError = obj.SelectToken("Err", errorWhenNoMatch: false);
            JToken ok = obj.SelectToken("Ok", errorWhenNoMatch: false);

            if (unexpectedError != null) {
                return new Core.CreateAccount.UnexpectedError {
                    errorMessage = result
                };
            }

            if (expectedError != null) {
                switch (expectedError.ToString()) {
                    case "InvalidUsername":
                        return new Core.CreateAccount.ExpectedError {
                            error = Core.CreateAccount.PossibleErrors.InvalidUsername
                        };
                    case "UsernameTaken":
                        return new Core.CreateAccount.ExpectedError {
                            error = Core.CreateAccount.PossibleErrors.UsernameTaken
                        };
                    case "CouldNotReachServer":
                        return new Core.CreateAccount.ExpectedError {
                            error = Core.CreateAccount.PossibleErrors.CouldNotReachServer
                        };
                    case "AccountExistsAlready":
                        return new Core.CreateAccount.ExpectedError {
                            error = Core.CreateAccount.PossibleErrors.AccountExistsAlready
                        };
                }
            }

            if (ok != null) {
                return new Core.CreateAccount.Success { };
            }

            return new Core.CreateAccount.UnexpectedError {
                errorMessage = "Contract error!"
            };
        }

        public static async Task<Core.GetAccount.Result> GetAccount() {
            String result = await Task.Run(() => {
                coreMutex.WaitOne();
                String coreRespose = getStringAndRelease(get_account(path));
                coreMutex.ReleaseMutex();
                return coreRespose;
            });


            JObject obj = JObject.Parse(result);

            JToken unexpectedError = obj.SelectToken("Err.UnexpectedError", errorWhenNoMatch: false);
            JToken expectedError = obj.SelectToken("Err", errorWhenNoMatch: false);
            JToken ok = obj.SelectToken("Ok", errorWhenNoMatch: false);

            if (unexpectedError != null) {
                return new Core.GetAccount.UnexpectedError {
                    errorMessage = result
                };
            }

            if (expectedError != null) {
                switch (expectedError.ToString()) {
                    case "NoAccount":
                        return new Core.GetAccount.ExpectedError {
                            error = Core.GetAccount.PossibleErrors.NoAccount
                        };
                };
            }

            if (ok != null) {
                return new Core.GetAccount.Success {
                    accountJson = ok.ToString()
                };
            }

            return new Core.GetAccount.UnexpectedError {
                errorMessage = "Contract error!"
            };
        }

        public static async Task<Core.ImportAccount.Result> ImportAccount(String account_string) {
            String result = await Task.Run(() => {
                coreMutex.WaitOne();
                String coreResponse = getStringAndRelease(import_account(path, account_string));
                coreMutex.ReleaseMutex();
                return coreResponse;
            });

            JObject obj = JObject.Parse(result);

            JToken unexpectedError = obj.SelectToken("Err.UnexpectedError", errorWhenNoMatch: false);
            JToken expectedError = obj.SelectToken("Err", errorWhenNoMatch: false);
            JToken ok = obj.SelectToken("Ok", errorWhenNoMatch: false);

            if (unexpectedError != null) {
                return new Core.ImportAccount.UnexpectedError {
                    errorMessage = result
                };
            }

            if (expectedError != null) {
                switch (expectedError.ToString()) {
                    case "AccountStringCorrupted":
                        return new Core.ImportAccount.ExpectedError {
                            error = Core.ImportAccount.PossibleErrors.AccountStringCorrupted
                        };
                    case "AccountExistsAlready":
                        return new Core.ImportAccount.ExpectedError {
                            error = Core.ImportAccount.PossibleErrors.AccountExistsAlready
                        };
                    case "AccountDoesNotExist":
                        return new Core.ImportAccount.ExpectedError {
                            error = Core.ImportAccount.PossibleErrors.AccountDoesNotExist
                        };
                    case "UsernamePKMismatch":
                        return new Core.ImportAccount.ExpectedError {
                            error = Core.ImportAccount.PossibleErrors.UsernamePKMismatch
                        };
                    case "CouldNotReachServer":
                        return new Core.ImportAccount.ExpectedError {
                            error = Core.ImportAccount.PossibleErrors.CouldNotReachServer
                        };
                }
            }

            if (ok != null) {
                return new Core.ImportAccount.Success { };
            }

            return new Core.ImportAccount.UnexpectedError {
                errorMessage = "Contract error!"
            };
        }

        public static async Task<Core.ListFileMetadata.Result> ListFileMetadata() {
            String result = await Task.Run(() => {
                coreMutex.WaitOne();
                String coreResult = getStringAndRelease(list_metadatas(path));
                coreMutex.ReleaseMutex();
                return coreResult;
            });

            JObject obj = JObject.Parse(result);

            JToken unexpectedError = obj.SelectToken("Err.UnexpectedError", errorWhenNoMatch: false);
            JToken ok = obj.SelectToken("Ok", errorWhenNoMatch: false);

            if (unexpectedError != null) {
                return new Core.ListFileMetadata.UnexpectedError {
                    errorMessage = result
                };
            }

            if (ok != null) {
                return new Core.ListFileMetadata.Success {
                    files = JsonConvert.DeserializeObject<List<FileMetadata>>(ok.ToString())
                };
            }

            return new Core.ListFileMetadata.UnexpectedError {
                errorMessage = result
            };

        }

        public static async Task<Core.CreateFile.Result> CreateFile(String name, String parent, FileType ft) {
            String fileType;

            if (ft == FileType.Folder) {
                fileType = "Folder";
            } else {
                fileType = "Document";
            }

            String result = await Task.Run(() => {
                coreMutex.WaitOne();
                String coreResponse = getStringAndRelease(create_file(path, name, parent, fileType));
                coreMutex.ReleaseMutex();
                return coreResponse;
            });

            JObject obj = JObject.Parse(result);

            JToken unexpectedError = obj.SelectToken("Err.UnexpectedError", errorWhenNoMatch: false);
            JToken expectedError = obj.SelectToken("Err", errorWhenNoMatch: false);
            JToken ok = obj.SelectToken("Ok", errorWhenNoMatch: false);

            if (unexpectedError != null) {
                return new Core.CreateFile.UnexpectedError {
                    errorMessage = result
                };
            }

            if (expectedError != null) {
                switch (expectedError.ToString()) {
                    case "NoAccount":
                        return new Core.CreateFile.ExpectedError {
                            error = Core.CreateFile.PossibleErrors.NoAccount
                        };
                    case "DocumentTreatedAsFolder":
                        return new Core.CreateFile.ExpectedError {
                            error = Core.CreateFile.PossibleErrors.DocumentTreatedAsFolder
                        };
                    case "CouldNotFindAParent":
                        return new Core.CreateFile.ExpectedError {
                            error = Core.CreateFile.PossibleErrors.CouldNotFindAParent
                        };
                    case "FileNameNotAvailable":
                        return new Core.CreateFile.ExpectedError {
                            error = Core.CreateFile.PossibleErrors.FileNameNotAvailable
                        };
                    case "FileNameContainsSlash":
                        return new Core.CreateFile.ExpectedError {
                            error = Core.CreateFile.PossibleErrors.FileNameContainsSlash
                        };
                    case "FileNameEmpty":
                        return new Core.CreateFile.ExpectedError {
                            error = Core.CreateFile.PossibleErrors.FileNameEmpty
                        };
                }
            }

            if (ok != null) {
                return new Core.CreateFile.Success {
                    NewFile = JsonConvert.DeserializeObject<FileMetadata>(ok.ToString())
                };
            }

            return new Core.CreateFile.UnexpectedError {
                errorMessage = "Contract error!"
            };
        }

        public static async Task<Core.SyncAll.Result> SyncAll() {
            String result = await Task.Run(() => {
                coreMutex.WaitOne();
                String coreResponse = getStringAndRelease(sync_all(path));
                coreMutex.ReleaseMutex();
                return coreResponse;
            });

            JObject obj = JObject.Parse(result);

            JToken unexpectedError = obj.SelectToken("Err.UnexpectedError", errorWhenNoMatch: false);
            JToken expectedError = obj.SelectToken("Err", errorWhenNoMatch: false);
            JToken ok = obj.SelectToken("Ok", errorWhenNoMatch: false);

            if (unexpectedError != null) {
                return new Core.SyncAll.UnexpectedError {
                    errorMessage = result
                };
            }

            if (expectedError != null) {
                switch (expectedError.ToString()) {
                    case "NoAccount":
                        return new Core.SyncAll.ExpectedError {
                            error = Core.SyncAll.PossibleErrors.NoAccount
                        };
                    case "CouldNotReachServer":
                        return new Core.SyncAll.ExpectedError {
                            error = Core.SyncAll.PossibleErrors.CouldNotReachServer
                        };
                    case "ExecuteWorkError": // TODO perhaps not how this works
                        return new Core.SyncAll.ExpectedError {
                            error = Core.SyncAll.PossibleErrors.ExecuteWorkError
                        };
                }
            }

            if (ok != null) {
                return new Core.SyncAll.Success { };
            }

            return new Core.SyncAll.UnexpectedError {
                errorMessage = "Contract error!"
            };
        }

        public static async Task<Core.RenameFile.Result> RenameFile(String id, String newName) {

            String result = await Task.Run(() => {
                coreMutex.WaitOne();
                String coreResponse = getStringAndRelease(rename_file(path, id, newName));
                coreMutex.ReleaseMutex();
                return coreResponse;
            });

            JObject obj = JObject.Parse(result);

            JToken unexpectedError = obj.SelectToken("Err.UnexpectedError", errorWhenNoMatch: false);
            JToken expectedError = obj.SelectToken("Err", errorWhenNoMatch: false);
            JToken ok = obj.SelectToken("Ok", errorWhenNoMatch: false);

            if (unexpectedError != null) {
                return new Core.RenameFile.UnexpectedError {
                    errorMessage = result
                };
            }

            if (expectedError != null) {
                switch (expectedError.ToString()) {
                    case "FileDoesNotExist":
                        return new Core.RenameFile.ExpectedError {
                            error = Core.RenameFile.PossibleErrors.FileDoesNotExist
                        };
                    case "NewNameContainsSlash":
                        return new Core.RenameFile.ExpectedError {
                            error = Core.RenameFile.PossibleErrors.NewNameContainsSlash
                        };
                    case "FileNameNotAvailable": // TODO perhaps not how this works
                        return new Core.RenameFile.ExpectedError {
                            error = Core.RenameFile.PossibleErrors.FileNameNotAvailable
                        };
                    case "NewNameEmpty":
                        return new Core.RenameFile.ExpectedError {
                            error = Core.RenameFile.PossibleErrors.NewNameEmpty
                        };
                    case "CannotRenameRoot":
                        return new Core.RenameFile.ExpectedError {
                            error = Core.RenameFile.PossibleErrors.CannotRenameRoot
                        };
                }
            }

            if (ok != null) {
                return new Core.RenameFile.Success { };
            }

            return new Core.RenameFile.UnexpectedError {
                errorMessage = "Contract error!"
            };
        }

        public static async Task<Core.MoveFile.Result> MoveFile(String id, String newParent) {

            String result = await Task.Run(() => {
                coreMutex.WaitOne();
                String coreResponse = getStringAndRelease(move_file(path, id, newParent));
                coreMutex.ReleaseMutex();
                return coreResponse;
            });

            JObject obj = JObject.Parse(result);

            JToken unexpectedError = obj.SelectToken("Err.UnexpectedError", errorWhenNoMatch: false);
            JToken expectedError = obj.SelectToken("Err", errorWhenNoMatch: false);
            JToken ok = obj.SelectToken("Ok", errorWhenNoMatch: false);

            if (unexpectedError != null) {
                return new Core.MoveFile.UnexpectedError {
                    errorMessage = result
                };
            }

            if (expectedError != null) {
                switch (expectedError.ToString()) {
                    case "NoAccount":
                        return new Core.MoveFile.ExpectedError {
                            error = Core.MoveFile.PossibleErrors.NoAccount
                        };
                    case "FileDoesNotExist":
                        return new Core.MoveFile.ExpectedError {
                            error = Core.MoveFile.PossibleErrors.FileDoesNotExist
                        };
                    case "DocumentTreatedAsFolder":
                        return new Core.MoveFile.ExpectedError {
                            error = Core.MoveFile.PossibleErrors.DocumentTreatedAsFolder
                        };
                    case "TargetParentHasChildNamedThat":
                        return new Core.MoveFile.ExpectedError {
                            error = Core.MoveFile.PossibleErrors.TargetParentHasChildNamedThat
                        };
                    case "TargetParentDoesNotExist":
                        return new Core.MoveFile.ExpectedError {
                            error = Core.MoveFile.PossibleErrors.TargetParentDoesNotExist
                        };
                    case "CannotMoveRoot":
                        return new Core.MoveFile.ExpectedError {
                            error = Core.MoveFile.PossibleErrors.CannotMoveRoot
                        };
                }
            }

            if (ok != null) {
                return new Core.MoveFile.Success { };
            }

            return new Core.MoveFile.UnexpectedError {
                errorMessage = "Contract error!"
            };
        }

        public static async Task<Core.ReadDocument.Result> ReadDocument(String id) {

            String result = await Task.Run(() => {
                coreMutex.WaitOne();
                String coreResponse = getStringAndRelease(read_document(path, id));
                coreMutex.ReleaseMutex();
                return coreResponse;
            });

            JObject obj = JObject.Parse(result);

            JToken unexpectedError = obj.SelectToken("Err.UnexpectedError", errorWhenNoMatch: false);
            JToken expectedError = obj.SelectToken("Err", errorWhenNoMatch: false);
            JToken ok = obj.SelectToken("Ok", errorWhenNoMatch: false);

            if (unexpectedError != null) {
                return new Core.ReadDocument.UnexpectedError {
                    errorMessage = result
                };
            }

            if (expectedError != null) {
                switch (expectedError.ToString()) {
                    case "NoAccount":
                        return new Core.ReadDocument.ExpectedError {
                            error = Core.ReadDocument.PossibleErrors.NoAccount
                        };
                    case "FileDoesNotExist":
                        return new Core.ReadDocument.ExpectedError {
                            error = Core.ReadDocument.PossibleErrors.FileDoesNotExist
                        };
                    case "TreatedFolderAsDocument":
                        return new Core.ReadDocument.ExpectedError {
                            error = Core.ReadDocument.PossibleErrors.TreatedFolderAsDocument
                        };
                }
            }

            if (ok != null) {
                return new Core.ReadDocument.Success {
                    content = JsonConvert.DeserializeObject<DecryptedValue>(ok.ToString())
                };
            }

            return new Core.ReadDocument.UnexpectedError {
                errorMessage = "Contract error!"
            };
        }

        public static async Task<Core.WriteDocument.Result> WriteDocument(String id, String content) {
            String result = await Task.Run(() => {
                coreMutex.WaitOne();
                String coreResponse = getStringAndRelease(write_document(path, id, content));
                coreMutex.ReleaseMutex();
                return coreResponse;
            });

            JObject obj = JObject.Parse(result);

            JToken unexpectedError = obj.SelectToken("Err.UnexpectedError", errorWhenNoMatch: false);
            JToken expectedError = obj.SelectToken("Err", errorWhenNoMatch: false);
            JToken ok = obj.SelectToken("Ok", errorWhenNoMatch: false);

            if (unexpectedError != null) {
                return new Core.WriteDocument.UnexpectedError {
                    errorMessage = result
                };
            }

            if (expectedError != null) {
                switch (expectedError.ToString()) {
                    case "NoAccount":
                        return new Core.WriteDocument.ExpectedError {
                            error = Core.WriteDocument.PossibleErrors.NoAccount
                        };
                    case "FileDoesNotExist":
                        return new Core.WriteDocument.ExpectedError {
                            error = Core.WriteDocument.PossibleErrors.FileDoesNotExist
                        };
                    case "TreatedFolderAsDocument":
                        return new Core.WriteDocument.ExpectedError {
                            error = Core.WriteDocument.PossibleErrors.TreatedFolderAsDocument
                        };
                }
            }

            if (ok != null) {
                return new Core.WriteDocument.Success { };
            }

            return new Core.WriteDocument.UnexpectedError {
                errorMessage = "Contract error!"
            };
        }

        public static async Task<Core.CalculateWork.Result> CalculateWork() {

            String result = await Task.Run(() => {
                coreMutex.WaitOne();
                String coreResponse = getStringAndRelease(calculate_work(path));
                coreMutex.ReleaseMutex();
                return coreResponse;
            });

            JObject obj = JObject.Parse(result);

            JToken unexpectedError = obj.SelectToken("Err.UnexpectedError", errorWhenNoMatch: false);
            JToken expectedError = obj.SelectToken("Err", errorWhenNoMatch: false);
            JToken ok = obj.SelectToken("Ok", errorWhenNoMatch: false);

            if (unexpectedError != null) {
                return new Core.CalculateWork.UnexpectedError {
                    errorMessage = result
                };
            }

            if (expectedError != null) {
                switch (expectedError.ToString()) {
                    case "NoAccount":
                        return new Core.CalculateWork.ExpectedError {
                            error = Core.CalculateWork.PossibleErrors.NoAccount
                        };
                    case "CouldNotReachServer":
                        return new Core.CalculateWork.ExpectedError {
                            error = Core.CalculateWork.PossibleErrors.CouldNotReachServer
                        };
                }
            }

            if (ok != null) {
                return new Core.CalculateWork.Success {
                    workCalculated = JsonConvert.DeserializeObject<Core.CalculateWork.WorkCalculated>(ok.ToString())
                };
            }

            return new Core.CalculateWork.UnexpectedError {
                errorMessage = "Contract error!"
            };
        }
    }
}
