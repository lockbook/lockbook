import Foundation
import SwiftUI
import SwiftWorkspace

struct ExportedURLDest {
    let id: UUID
    let destination: URL
}

class ImportExportService: ObservableObject {
    let core: Lb

    init(_ core: Lb) {
        self.core = core
    }
    
    static let TMP_DIR = "lb-tmp"
    
    public func importFilesSync(sources: [String], destination: UUID) -> Bool {
        print("importing files")
        let operation = DI.core.importFiles(sources: sources, dest: destination)

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
        guard let destination = createTempTempDir() else {
            return nil
        }
        

        let operation = DI.core.exportFile(sourceId: meta.id, dest: destination.path(), edit: true)
        
        switch operation {
        case .success(_):
            return destination.appendingPathComponent(meta.name)
        case .failure(let error):
            DI.errors.handleError(error)
            return nil
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
