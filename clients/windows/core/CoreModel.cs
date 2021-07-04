using Newtonsoft.Json;
using System;
using System.Collections.Generic;

namespace Core {
    public abstract class ExpectedError<T> where T : Enum {
        public T Error { get; set; }
    }

    public static class ExpectedErrors {
        public static TErr New<TIResult, TErr, TPossibleErrs>(TPossibleErrs value)
            where TErr : ExpectedError<TPossibleErrs>, TIResult, new()
            where TPossibleErrs : Enum {
            return new TErr { Error = value };
        }
    }

    public abstract class UnexpectedError {
        public string ErrorMessage { get; set; }
    }

    public static class UnexpectedErrors {
        public static TErr New<TIResult, TErr>(string value)
            where TErr : UnexpectedError, TIResult, new() {
            return new TErr { ErrorMessage = value };
        }
    }

    public class ClientFileMetadata {
        [JsonProperty("id")]
        public string Id;
        [JsonProperty("name")]
        public string Name;
        [JsonProperty("parent")]
        public string Parent;
        [JsonProperty("file_type")]
        public string Type;
        [JsonProperty("deleted")]
        public bool deleted;
    }

    public class Account {
        [JsonProperty("username")]
        public string username;
        [JsonProperty("api_url")]
        public string apiUrl;
        [JsonProperty("private_key")]
        public byte[] key;
    }

    public enum FileType {
        Folder,
        Document
    }

    public enum DbState {
        ReadyToUse,
        Empty,
        MigrationRequired,
        StateRequiresClearing,
    }

    public class UsageMetrics {
        [JsonProperty("usages")]
        public List<FileUsage> usages;
        [JsonProperty("server_usage")]
        public UsageItemMetric server_usage;
        [JsonProperty("data_cap")]
        public UsageItemMetric data_cap;
}

    public class UsageItemMetric {
        [JsonProperty("exact")]
        public ulong exact;
        [JsonProperty("readable")]
        public string readable;
    }

    public class FileUsage {
        [JsonProperty("file_id")]
        public string fileId;
        [JsonProperty("size_bytes")]
        public ulong sizeBytes;
    }

    public class ClientWorkCalculated {
        [JsonProperty("local_files")]
        public List<ClientFileMetadata> localFiles;
        [JsonProperty("server_files")]
        public List<ClientFileMetadata> serverFiles;
        [JsonProperty("server_unknown_name_count")]
        public ulong serverUnknownNameCount;
        [JsonProperty("most_recent_update_from_server")]
        public ulong mostRecentUpdateFromServer;
    }

    namespace GetDbState {
        public interface IResult { }
        public class Success : IResult {
            public DbState dbState;
        }
        public enum PossibleErrors {
            Stub,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace MigrateDb {
        public interface IResult { }
        public class Success : IResult { }
        public enum PossibleErrors {
            StateRequiresCleaning,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace CreateAccount {
        public interface IResult { }
        public class Success : IResult { }
        public enum PossibleErrors {
            UsernameTaken,
            InvalidUsername,
            CouldNotReachServer,
            AccountExistsAlready,
            ClientUpdateRequired,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace ImportAccount {
        public interface IResult { }
        public class Success : IResult { }
        public enum PossibleErrors {
            AccountStringCorrupted,
            AccountExistsAlready,
            UsernamePKMismatch,
            CouldNotReachServer,
            AccountDoesNotExist,
            ClientUpdateRequired,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace ExportAccount {
        public interface IResult { }
        public class Success : IResult {
            public string accountString;
        }
        public enum PossibleErrors {
            NoAccount,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace GetAccount {
        public interface IResult { }
        public class Success : IResult {
            public Account account;
        }
        public enum PossibleErrors {
            NoAccount
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace CreateFileAtPath {
        public interface IResult { }
        public class Success : IResult {
            public ClientFileMetadata newFile;
        }
        public enum PossibleErrors {
            PathDoesntStartWithRoot,
            PathContainsEmptyFile,
            FileAlreadyExists,
            NoRoot,
            NoAccount,
            DocumentTreatedAsFolder,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace WriteDocument {
        public interface IResult { }
        public class Success : IResult { }
        public enum PossibleErrors {
            NoAccount,
            FolderTreatedAsDocument,
            FileDoesNotExist
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace CreateFile {
        public interface IResult { }
        public class Success : IResult {
            public ClientFileMetadata newFile;
        }
        public enum PossibleErrors {
            NoAccount,
            DocumentTreatedAsFolder,
            CouldNotFindAParent,
            FileNameNotAvailable,
            FileNameContainsSlash,
            FileNameEmpty,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace GetRoot {
        public interface IResult { }
        public class Success : IResult {
            public ClientFileMetadata root;
        }
        public enum PossibleErrors {
            NoRoot,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace GetChildren {
        public interface IResult { }
        public class Success : IResult {
            public List<ClientFileMetadata> children;
        }
        public enum PossibleErrors {
            Stub,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace ReadDocument {
        public interface IResult { }
        public class Success : IResult {
            public string content;
        }
        public enum PossibleErrors {
            TreatedFolderAsDocument,
            NoAccount,
            FileDoesNotExist,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace GetFileByPath {
        public interface IResult { }
        public class Success : IResult {
            public ClientFileMetadata file;
        }
        public enum PossibleErrors {
            NoFileAtThatPath,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace DeleteFile {
        public interface IResult { }
        public class Success : IResult {
        }
        public enum PossibleErrors {
            FileDoesNotExist,
            CannotDeleteRoot,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace ListPaths {
        public interface IResult { }
        public class Success : IResult {
            public List<string> paths;
        }
        public enum PossibleErrors {
            Stub,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace ListMetadatas {
        public interface IResult { }
        public class Success : IResult {
            public List<ClientFileMetadata> files;
        }
        public enum PossibleErrors {
            Stub,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace RenameFile {
        public interface IResult { }
        public class Success : IResult { }
        public enum PossibleErrors {
            FileDoesNotExist,
            NewNameContainsSlash,
            FileNameNotAvailable,
            NewNameEmpty,
            CannotRenameRoot,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace MoveFile {
        public interface IResult { }
        public class Success : IResult { }
        public enum PossibleErrors {
            NoAccount,
            FileDoesNotExist,
            DocumentTreatedAsFolder,
            TargetParentHasChildNamedThat,
            TargetParentDoesNotExist,
            CannotMoveRoot,
            FolderMovedIntoItself,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace SyncAll {
        public interface IResult { }
        public class Success : IResult { }
        public enum PossibleErrors {
            NoAccount,
            ClientUpdateRequired,
            CouldNotReachServer,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace CalculateWork {
        public interface IResult { }
        public class Success : IResult {
            public ClientWorkCalculated workCalculated;
        }
        public enum PossibleErrors {
            NoAccount,
            CouldNotReachServer,
            ClientUpdateRequired,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace SetLastSynced {
        public interface IResult { }
        public class Success : IResult { }
        public enum PossibleErrors {
            Stub,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace GetLastSynced {
        public interface IResult { }
        public class Success : IResult {
            public ulong timestamp;
        }
        public enum PossibleErrors {
            Stub,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace GetLastSyncedHumanString {
        public interface IResult { }
        public class Success : IResult {
            public string timestamp;
        }
        public enum PossibleErrors {
            Stub,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace GetUsage {
        public interface IResult { }
        public class Success : IResult {
            public UsageMetrics usage;
        }
        public enum PossibleErrors {
            NoAccount,
            CouldNotReachServer,
            ClientUpdateRequired,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace GetDrawing {
            public interface IResult { }
            public class Success : IResult {
                public string content;
            }
            public enum PossibleErrors {
                NoAccount,
                FolderTreatedAsDrawing,
                InvalidDrawing,
                FileDoesNotExist,
            }
            public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
            public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace SaveDrawing {
            public interface IResult { }
            public class Success : IResult {
                public string content;
            }
            public enum PossibleErrors {
                NoAccount,
                FileDoesNotExist,
                FolderTreatedAsDrawing,
                InvalidDrawing,
            }
            public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
            public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace ExportDrawing {
            public interface IResult { }
            public class Success : IResult {
                public string content;
            }
            public enum PossibleErrors {
                FolderTreatedAsDrawing,
                FileDoesNotExist,
                NoAccount,
                InvalidDrawing,
            }
            public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
            public class UnexpectedError : Core.UnexpectedError, IResult { }
    }
}
