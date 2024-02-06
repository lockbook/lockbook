import XCTest
@testable import SwiftLockbookCore
@testable import CLockbookCore

class ErrorEnumTests: XCTestCase {
    struct AllErrors: Decodable {
        let CreateAccountError: [CreateAccountError]
        let ImportError: [ImportError]
        let AccountExportError: [AccountExportError]
        let GetAccountError: [GetAccountError]
        let CreateFileAtPathError: [CreateFileAtPathError]
        let WriteToDocumentError: [WriteToDocumentError]
        let CreateFileError: [CreateFileError]
        let GetRootError: [GetRootError]
        let GetFileByIdError: [GetFileByIdError]
        let GetFileByPathError: [GetFileByPathError]
        let ReadDocumentError: [ReadDocumentError]
        let RenameFileError: [RenameFileError]
        let MoveFileError: [MoveFileError]
        let SyncAllError: [SyncAllError]
        let CalculateWorkError: [CalculateWorkError]
        let GetUsageError: [GetUsageError]
        let FileDeleteError: [FileDeleteError]
        let GetDrawingError: [GetDrawingError]
        let SaveDrawingError: [SaveDrawingError]
        let ExportDrawingError: [ExportDrawingError]

        func noneEmpty() -> Bool {
                !CreateAccountError.isEmpty
                && !ImportError.isEmpty
                && !AccountExportError.isEmpty
                && !GetAccountError.isEmpty
                && !CreateFileAtPathError.isEmpty
                && !WriteToDocumentError.isEmpty
                && !CreateFileError.isEmpty
                && !GetRootError.isEmpty
                && !GetFileByIdError.isEmpty
                && !GetFileByPathError.isEmpty
                && !ReadDocumentError.isEmpty
                && !RenameFileError.isEmpty
                && !MoveFileError.isEmpty
                && !SyncAllError.isEmpty
                && !CalculateWorkError.isEmpty
                && !GetUsageError.isEmpty
                && !FileDeleteError.isEmpty
                && !GetDrawingError.isEmpty
                && !SaveDrawingError.isEmpty
                && !ExportDrawingError.isEmpty
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

