using Newtonsoft.Json;
using System.Collections.Generic;

namespace Core {
    public interface IExpectedError<T> {
        public T Error { get; set; }
    }

    public interface IUnexpectedError {
        public string ErrorMessage { get; set; }
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
        public string fileId;
        public ulong byteSeconds;
        public ulong seconds;
    }

    public class WorkCalculated {
        [JsonProperty("work_units")]
        public List<dynamic> WorkUnits;

        [JsonProperty("most_recent_update_from_server")]
        public ulong MostRecentUpdateFromServer;
    }

    namespace GetDbState {
        public abstract class Result { }

        public class Success : Result {
            public DbState dbState;
        }

        public class UnexpectedError : Result, IUnexpectedError {
            public string ErrorMessage { get; set; }
        }
    }

    namespace MigrateDb {
        public abstract class Result { }

        public class Success : Result { }

        public enum PossibleErrors {
            ClientUpdateRequired,
        }

        public class ExpectedError : Result, IExpectedError<PossibleErrors> {
            public PossibleErrors Error { get; set; }
        }

        public class UnexpectedError : Result, IUnexpectedError {
            public string ErrorMessage { get; set; }
        }
    }

    namespace CreateAccount {
        public abstract class Result { }

        public class Success : Result { }

        public enum PossibleErrors {
            UsernameTaken,
            InvalidUsername,
            CouldNotReachServer,
            AccountExistsAlready,
            ClientUpdateRequired,
        }

        public class ExpectedError : Result, IExpectedError<PossibleErrors> {
            public PossibleErrors Error { get; set; }
        }

        public class UnexpectedError : Result, IUnexpectedError {
            public string ErrorMessage { get; set; }
        }
    }

    namespace ImportAccount {
        public abstract class Result { }

        public class Success : Result { }

        public enum PossibleErrors {
            AccountStringCorrupted,
            AccountExistsAlready,
            UsernamePKMismatch,
            CouldNotReachServer,
            AccountDoesNotExist,
            ClientUpdateRequired,
        }

        public class ExpectedError : Result, IExpectedError<PossibleErrors> {
            public PossibleErrors Error { get; set; }
        }

        public class UnexpectedError : Result, IUnexpectedError {
            public string ErrorMessage { get; set; }
        }
    }

    namespace ExportAccount {
        public abstract class Result { }

        public class Success : Result {
            string accountString;
        }

        public enum PossibleErrors {
            NoAccount,
        }

        public class ExpectedError : Result, IExpectedError<PossibleErrors> {
            public PossibleErrors Error { get; set; }
        }

        public class UnexpectedError : Result, IUnexpectedError {
            public string ErrorMessage { get; set; }
        }
    }

    namespace GetAccount {
        public abstract class Result { }

        public class Success : Result {
            public string accountJson;
        }

        public enum PossibleErrors {
            NoAccount
        }

        public class ExpectedError : Result, IExpectedError<PossibleErrors> {
            public PossibleErrors Error { get; set; }
        }

        public class UnexpectedError : Result, IUnexpectedError {
            public string ErrorMessage { get; set; }
        }
    }

    namespace CreateFileAtPath {
        public abstract class Result { }

        public class Success : Result {
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

        public class ExpectedError : Result, IExpectedError<PossibleErrors> {
            public PossibleErrors Error { get; set; }
        }

        public class UnexpectedError : Result, IUnexpectedError {
            public string ErrorMessage { get; set; }
        }
    }

    namespace WriteDocument {
        public abstract class Result { }

        public class Success : Result { }

        public enum PossibleErrors {
            NoAccount,
            FolderTreatedAsDocument,
            FileDoesNotExist
        }

        public class ExpectedError : Result, IExpectedError<PossibleErrors> {
            public PossibleErrors Error { get; set; }
        }

        public class UnexpectedError : Result, IUnexpectedError {
            public string ErrorMessage { get; set; }
        }
    }

    namespace CreateFile {
        public abstract class Result { }

        public class Success : Result {
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

        public class ExpectedError : Result, IExpectedError<PossibleErrors> {
            public PossibleErrors Error { get; set; }
        }


        public class UnexpectedError : Result, IUnexpectedError {
            public string ErrorMessage { get; set; }
        }
    }

    namespace GetRoot {
        public abstract class Result { }

        public class Success : Result {
            public FileMetadata root;
        }

        public enum PossibleErrors {
            NoRoot,
        }

        public class ExpectedError : Result, IExpectedError<PossibleErrors> {
            public PossibleErrors Error { get; set; }
        }


        public class UnexpectedError : Result, IUnexpectedError {
            public string ErrorMessage { get; set; }
        }
    }

    namespace GetChildren {
        public abstract class Result { }

        public class Success : Result {
            public List<FileMetadata> children;
        }

        public class UnexpectedError : Result, IUnexpectedError {
            public string ErrorMessage { get; set; }
        }
    }

    namespace GetFileById {
        public abstract class Result { }

        public class Success : Result {
            public FileMetadata file;
        }

        public enum PossibleErrors {
            NoFileWithThatId,
        }

        public class ExpectedError : Result, IExpectedError<PossibleErrors> {
            public PossibleErrors Error { get; set; }
        }


        public class UnexpectedError : Result, IUnexpectedError {
            public string ErrorMessage { get; set; }
        }
    }

    namespace GetFileByPath {
        public abstract class Result { }

        public class Success : Result {
            public FileMetadata file;
        }

        public enum PossibleErrors {
            NoFileAtThatPath,
        }

        public class ExpectedError : Result, IExpectedError<PossibleErrors> {
            public PossibleErrors Error { get; set; }
        }

        public class UnexpectedError : Result, IUnexpectedError {
            public string ErrorMessage { get; set; }
        }
    }

    namespace InsertFile {
        public abstract class Result { }

        public class Success : Result { }

        public class UnexpectedError : Result, IUnexpectedError {
            public string ErrorMessage { get; set; }
        }
    }

    namespace DeleteFile {
        public abstract class Result { }

        public class Success : Result {
        }

        public enum PossibleErrors {
            NoFileWithThatId,
        }

        public class ExpectedError : Result, IExpectedError<PossibleErrors> {
            public PossibleErrors Error { get; set; }
        }

        public class UnexpectedError : Result, IUnexpectedError {
            public string ErrorMessage { get; set; }
        }
    }

    namespace ReadDocument {
        public abstract class Result { }

        public class Success : Result {
            public DecryptedValue content;
        }

        public enum PossibleErrors {
            TreatedFolderAsDocument,
            NoAccount,
            FileDoesNotExist,
        }

        public class ExpectedError : Result, IExpectedError<PossibleErrors> {
            public PossibleErrors Error { get; set; }
        }

        public class UnexpectedError : Result, IUnexpectedError {
            public string ErrorMessage { get; set; }
        }
    }

    namespace ListPaths {
        public abstract class Result { }

        public class Success : Result {
            public List<string> paths;
        }

        public class UnexpectedError : Result, IUnexpectedError {
            public string ErrorMessage { get; set; }
        }
    }

    namespace ListMetadatas {
        public abstract class Result { }

        public class Success : Result {
            public List<FileMetadata> files;
        }

        public class UnexpectedError : Result, IUnexpectedError {
            public string ErrorMessage { get; set; }
        }
    }

    namespace RenameFile {
        public abstract class Result { }

        public class Success : Result { }

        public enum PossibleErrors {
            FileDoesNotExist,
            NewNameContainsSlash,
            FileNameNotAvailable,
            NewNameEmpty,
            CannotRenameRoot
        }

        public class ExpectedError : Result, IExpectedError<PossibleErrors> {
            public PossibleErrors Error { get; set; }
        }

        public class UnexpectedError : Result, IUnexpectedError {
            public string ErrorMessage { get; set; }
        }
    }

    namespace MoveFile {
        public abstract class Result { }

        public class Success : Result { }

        public enum PossibleErrors {
            NoAccount,
            FileDoesNotExist,
            DocumentTreatedAsFolder,
            TargetParentHasChildNamedThat,
            TargetParentDoesNotExist,
            CannotMoveRoot,

        }

        public class ExpectedError : Result, IExpectedError<PossibleErrors> {
            public PossibleErrors Error { get; set; }
        }

        public class UnexpectedError : Result, IUnexpectedError {
            public string ErrorMessage { get; set; }
        }
    }

    namespace SyncAll {
        public abstract class Result { }

        public class Success : Result { }

        public enum PossibleErrors {
            NoAccount,
            CouldNotReachServer,
            ExecuteWorkError,
        }

        public class ExpectedError : Result, IExpectedError<PossibleErrors> {
            public PossibleErrors Error { get; set; }
        }

        public class UnexpectedError : Result, IUnexpectedError {
            public string ErrorMessage { get; set; }
        }
    }

    namespace CalculateWork {
        public abstract class Result { }

        public class Success : Result {
            public WorkCalculated workCalculated;
        }

        public enum PossibleErrors {
            NoAccount,
            CouldNotReachServer,
            ClientUpdateRequired,
        }

        public class ExpectedError : Result, IExpectedError<PossibleErrors> {
            public PossibleErrors Error { get; set; }
        }

        public class UnexpectedError : Result, IUnexpectedError {
            public string ErrorMessage { get; set; }
        }
    }

    namespace ExecuteWork {
        public abstract class Result { }

        public class Success : Result { }

        public enum PossibleErrors {
            CouldNotReachServer,
            ClientUpdateRequired,
        }

        public class ExpectedError : Result, IExpectedError<PossibleErrors> {
            public PossibleErrors Error { get; set; }
        }

        public class UnexpectedError : Result, IUnexpectedError {
            public string ErrorMessage { get; set; }
        }
    }

    namespace SetLastSynced {
        public abstract class Result { }

        public class Success : Result { }

        public class UnexpectedError : Result, IUnexpectedError {
            public string ErrorMessage { get; set; }
        }
    }

    namespace GetLastSynced {
        public abstract class Result { }

        public class Success : Result {
            public ulong timestamp;
        }

        public class UnexpectedError : Result, IUnexpectedError {
            public string ErrorMessage { get; set; }
        }
    }

    namespace GetUsage {
        public abstract class Result { }

        public class Success : Result {
            public List<FileUsage> usage;
        }

        public enum PossibleErrors {
            NoAccount,
            CouldNotReachServer,
            ClientUpdateRequired,
        }

        public class ExpectedError : Result, IExpectedError<PossibleErrors> {
            public PossibleErrors Error { get; set; }
        }

        public class UnexpectedError : Result, IUnexpectedError {
            public string ErrorMessage { get; set; }
        }
    }
}

