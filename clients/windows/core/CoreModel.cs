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

    public class FileMetadata {
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

    public class DecryptedValue {
        [JsonProperty("secret")]
        public string secret;
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

    public class FileUsage {
        [JsonProperty("file_id")]
        public string fileId;
        [JsonProperty("byte_secs")]
        public ulong byteSeconds;
        [JsonProperty("secs")]
        public ulong seconds;
    }

    public class WorkCalculated {
        [JsonProperty("work_units")]
        public List<dynamic> workUnits;
        [JsonProperty("most_recent_update_from_server")]
        public ulong mostRecentUpdateFromServer;
    }

    namespace GetDbState {
        public interface IResult { }
        public class Success : IResult {
            public DbState dbState;
        }
        public enum PossibleErrors { }
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
            public string accountJson;
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
            public FileMetadata newFile;
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
            public FileMetadata newFile;
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
            public FileMetadata root;
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
            public List<FileMetadata> children;
        }
        public enum PossibleErrors { }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace ReadDocument {
        public interface IResult { }
        public class Success : IResult {
            public DecryptedValue content;
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
            public FileMetadata file;
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
        public enum PossibleErrors { }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace ListMetadatas {
        public interface IResult { }
        public class Success : IResult {
            public List<FileMetadata> files;
        }
        public enum PossibleErrors { }
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
            CannotRenameRoot
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
            CouldNotReachServer,
            ExecuteWorkError,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace CalculateWork {
        public interface IResult { }
        public class Success : IResult {
            public WorkCalculated workCalculated;
        }
        public enum PossibleErrors {
            NoAccount,
            CouldNotReachServer,
            ClientUpdateRequired,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace ExecuteWork {
        public interface IResult { }
        public class Success : IResult { }
        public enum PossibleErrors {
            CouldNotReachServer,
            ClientUpdateRequired,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace SetLastSynced {
        public interface IResult { }
        public class Success : IResult { }
        public enum PossibleErrors { }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace GetLastSynced {
        public interface IResult { }
        public class Success : IResult {
            public ulong timestamp;
        }
        public enum PossibleErrors { }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace GetUsage {
        public interface IResult { }
        public class Success : IResult {
            public List<FileUsage> usage;
        }
        public enum PossibleErrors {
            NoAccount,
            CouldNotReachServer,
            ClientUpdateRequired,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }
}
