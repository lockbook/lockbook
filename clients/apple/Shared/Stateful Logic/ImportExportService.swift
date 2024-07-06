import Foundation
import SwiftUI
import SwiftLockbookCore

struct ExportedURLDest {
    let id: UUID
    let destination: URL
}

class ImportExportService: ObservableObject {
    let core: LockbookApi

    init(_ core: LockbookApi) {
        self.core = core
    }
    
    static let TMP_DIR = "lb-tmp"
    
    public func importFilesSync(sources: [String], destination: UUID) -> Bool {
        print("importing files")
        let operation = DI.core.importFiles(sources: sources, destination: destination)

        switch operation {
        case .success(_):
            DI.files.successfulAction = .importFiles
            DI.files.refresh()
            
            return true
        case .failure(let error):
            DI.errors.handleError(error)
            return false
        }
    }
    
    public func exportFilesToTempDirSync(meta: File) -> URL? {
        guard var destination = createTempTempDir() else {
            return nil
        }
        
        if meta.fileType == .Document && meta.name.hasSuffix(".draw") {
            destination = destination.appendingPathComponent(meta.name + ".jpeg")
            let operation = DI.core.exportDrawingToDisk(id: meta.id, destination: destination.path())
            
            switch operation {
            case .success(_):
                return destination
            case .failure(let error):
                DI.errors.handleError(error)
                return nil
            }
        } else {
            let operation = DI.core.exportFile(id: meta.id, destination: destination.path())
            
            switch operation {
            case .success(_):
                return destination.appendingPathComponent(meta.name)
            case .failure(let error):
                DI.errors.handleError(error)
                return nil
            }
        }
    }

    func createTempTempDir() -> URL? {
        let fileManager = FileManager.default
        let tempTempURL = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent(ImportExportService.TMP_DIR).appendingPathComponent(UUID().uuidString)
        
        do {
            try fileManager.createDirectory(at: tempTempURL, withIntermediateDirectories: true, attributes: nil)
        } catch {
            return nil
        }
        
        return tempTempURL
    }

}
