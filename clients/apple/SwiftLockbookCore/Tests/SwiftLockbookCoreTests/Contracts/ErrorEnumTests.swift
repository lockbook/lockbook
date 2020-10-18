import XCTest
@testable import SwiftLockbookCore
@testable import CLockbookCore

class ErrorEnumTests: XCTestCase {
    struct AllErrors: Decodable {
        let GetStateError: [GetStateError]
        let MigrationError: [MigrationError]
        let CreateAccountError: [CreateAccountError]
        let ImportError: [ImportError]
        let AccountExportError: [AccountExportError]
        let GetAccountError: [GetAccountError]
        let CreateFileAtPathError: [CreateFileAtPathError]
        let WriteToDocumentError: [WriteToDocumentError]
        let CreateFileError: [CreateFileError]
        let GetRootError: [GetRootError]
        let GetChildrenError: [GetChildrenError]
        let GetFileByIdError: [GetFileByIdError]
        let GetFileByPathError: [GetFileByPathError]
        let InsertFileError: [InsertFileError]
        let DeleteFileError: [DeleteFileError]
        let ReadDocumentError: [ReadDocumentError]
        let ListPathsError: [ListPathsError]
        let ListMetadatasError: [ListMetadatasError]
        let RenameFileError: [RenameFileError]
        let MoveFileError: [MoveFileError]
        let SyncAllError: [SyncAllError]
        let CalculateWorkError: [CalculateWorkError]
        let ExecuteWorkError: [ExecuteWorkError]
        let SetLastSyncedError: [SetLastSyncedError]
        let GetLastSyncedError: [GetLastSyncedError]
        let GetUsageError: [GetUsageError]
        
        func noneEmpty() -> Bool {
            !GetStateError.isEmpty
                && !MigrationError.isEmpty
                && !CreateAccountError.isEmpty
                && !ImportError.isEmpty
                && !AccountExportError.isEmpty
                && !GetAccountError.isEmpty
                && !CreateFileAtPathError.isEmpty
                && !WriteToDocumentError.isEmpty
                && !CreateFileError.isEmpty
                && !GetRootError.isEmpty
                && !GetChildrenError.isEmpty
                && !GetFileByIdError.isEmpty
                && !GetFileByPathError.isEmpty
                && !InsertFileError.isEmpty
                && !DeleteFileError.isEmpty
                && !ReadDocumentError.isEmpty
                && !ListPathsError.isEmpty
                && !ListMetadatasError.isEmpty
                && !RenameFileError.isEmpty
                && !MoveFileError.isEmpty
                && !SyncAllError.isEmpty
                && !CalculateWorkError.isEmpty
                && !ExecuteWorkError.isEmpty
                && !SetLastSyncedError.isEmpty
                && !GetLastSyncedError.isEmpty
                && !GetUsageError.isEmpty
        }
    }
    
    func testAllVariants() throws {
        let result = get_variants()!
        let resultString = String(cString: result)
        release_pointer(UnsafeMutablePointer(mutating: result))
        
        let allErrors: AllErrors = try deserialize(data: resultString.data(using: .utf8)!).get()
        
        XCTAssert(allErrors.noneEmpty(), "Some error variants are empty!")
    }
}
