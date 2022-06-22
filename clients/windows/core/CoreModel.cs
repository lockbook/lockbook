using Newtonsoft.Json;
using Newtonsoft.Json.Converters;
using Newtonsoft.Json.Serialization;
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

    public class DecryptedFileMetadata {
        [JsonProperty("id", Required = Required.Always)]
        public string id;
        [JsonProperty("file_type", Required = Required.Always)]
        public string fileType;
        [JsonProperty("parent", Required = Required.Always)]
        public string parent;
        [JsonProperty("decrypted_name", Required = Required.Always)]
        public string decryptedName;
        [JsonProperty("owner", Required = Required.Always)]
        public string owner;
        [JsonProperty("metadata_version", Required = Required.Always)]
        public ulong metadataVersion;
        [JsonProperty("content_version", Required = Required.Always)]
        public ulong contentVersion;
        [JsonProperty("deleted", Required = Required.Always)]
        public bool deleted;
        [JsonProperty("decrypted_access_key", Required = Required.Always)]
        public List<byte> decrypted_access_key;
    }

    public class Account {
        [JsonProperty("username", Required = Required.Always)]
        public string username;
        [JsonProperty("api_url", Required = Required.Always)]
        public string apiUrl;
        [JsonProperty("private_key", Required = Required.Always)]
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
        [JsonProperty("usages", Required = Required.Always)]
        public List<FileUsage> usages;
        [JsonProperty("server_usage", Required = Required.Always)]
        public UsageItemMetric server_usage;
        [JsonProperty("data_cap", Required = Required.Always)]
        public UsageItemMetric data_cap;
    }

    public class UsageItemMetric {
        [JsonProperty("exact", Required = Required.Always)]
        public ulong exact;
        [JsonProperty("readable", Required = Required.Always)]
        public string readable;
    }

    public class FileUsage {
        [JsonProperty("file_id", Required = Required.Always)]
        public string fileId;
        [JsonProperty("size_bytes", Required = Required.Always)]
        public ulong sizeBytes;
    }

    public class WorkCalculated {
        [JsonProperty("work_units", Required = Required.Always)]
        public List<WorkUnit> workUnits;
        [JsonProperty("most_recent_update_from_server", Required = Required.Always)]
        public ulong mostRecentUpdateFromServer;
    }

    public class WorkUnit {
        [JsonProperty("content", Required = Required.Always)]
        public WorkUnitStructVariant content;
        [JsonProperty("tag", Required = Required.Always)]
        public string tag;
    }

    public class WorkUnitStructVariant {
        [JsonProperty("metadata", Required = Required.Always)]
        public DecryptedFileMetadata content;
    }

    public class Drawing {
        [JsonProperty("scale", Required = Required.Always)]
        public float scale;
        [JsonProperty("translation_x", Required = Required.Always)]
        public float translationX;
        [JsonProperty("translation_y", Required = Required.Always)]
        public float translationY;
        [JsonProperty("strokes", Required = Required.Always)]
        public List<Stroke> strokes;
        [JsonProperty("theme", Required = Required.Default)]
        public Dictionary<ColorAlias, ColorRGB> theme;
    }

    public class Stroke {
        [JsonProperty("points_x", Required = Required.Always)]
        public List<float> pointsX;
        [JsonProperty("points_y", Required = Required.Always)]
        public List<float> pointsY;
        [JsonProperty("points_girth", Required = Required.Always)]
        public List<float> pointsGirth;
        [JsonProperty("color", Required = Required.Always)]
        public ColorAlias color;
        [JsonProperty("alpha", Required = Required.Always)]
        public float alpha;
    }

    [JsonConverter(typeof(StringEnumConverter))]
    public enum ColorAlias {
        Black,
        Red,
        Green,
        Yellow,
        Blue,
        Magenta,
        Cyan,
        White,
    }

    public class ColorRGB {
        public byte r;
        public byte g;
        public byte b;


        public override bool Equals(object obj) {
            if(obj is ColorRGB other) {
                return r == other.r && g == other.g && b == other.b;
            }
            return false;
        }

        public override int GetHashCode() {
            return r << 16 | g << 8 | b;
        }
    }

    namespace Init {
        public interface IResult { }
        public class Success : IResult { }
        public enum PossibleErrors {
            Stub,
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
            ServerDisabled,
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
            public DecryptedFileMetadata newFile;
        }
        public enum PossibleErrors {
            PathDoesntStartWithRoot,
            PathContainsEmptyFile,
            FileAlreadyExists,
            NoRoot,
            DocumentTreatedAsFolder,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace WriteDocument {
        public interface IResult { }
        public class Success : IResult { }
        public enum PossibleErrors {
            FolderTreatedAsDocument,
            FileDoesNotExist
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace CreateFile {
        public interface IResult { }
        public class Success : IResult {
            public DecryptedFileMetadata newFile;
        }
        public enum PossibleErrors {
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
            public DecryptedFileMetadata root;
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
            public List<DecryptedFileMetadata> children;
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
            FileDoesNotExist,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }

    namespace GetByPath {
        public interface IResult { }
        public class Success : IResult {
            public DecryptedFileMetadata file;
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
            public List<DecryptedFileMetadata> files;
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
            ClientUpdateRequired,
            CouldNotReachServer,
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
            CouldNotReachServer,
            ClientUpdateRequired,
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
            InvalidDrawing,
        }
        public class ExpectedError : ExpectedError<PossibleErrors>, IResult { }
        public class UnexpectedError : Core.UnexpectedError, IResult { }
    }
}
