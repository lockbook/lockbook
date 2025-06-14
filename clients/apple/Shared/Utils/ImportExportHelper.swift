import SwiftUI
import SwiftWorkspace

class ImportExportHelper {
    static let TMP_DIR = "lb-tmp"
    
    static public func importFiles(homeState: HomeState, filesModel: FilesViewModel, sources: [String], destination: UUID) -> Bool {
        let operation = AppState.lb.importFiles(sources: sources, dest: destination)

        switch operation {
        case .success(_):
            homeState.fileActionCompleted = .importFiles
            filesModel.loadFiles()
            
            return true
        case .failure(let err):
            AppState.shared.error = .lb(error: err)
            
            return false
        }
    }
    
    static public func exportFilesToTempDir(homeState: HomeState, file: File) -> URL? {
        guard let destination = ImportExportHelper.createTempTempDir() else {
            return nil
        }
        

        let res = AppState.lb.exportFile(sourceId: file.id, dest: destination.path(), edit: true)
        
        switch res {
        case .success(_):
            return destination.appendingPathComponent(file.name)
        case .failure(let err):
            AppState.shared.error = .lb(error: err)
            
            return nil
        }
    }
    
    static func createTempTempDir() -> URL? {
        let fileManager = FileManager.default
        let tempTempURL = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent(ImportExportHelper.TMP_DIR).appendingPathComponent(UUID().uuidString)
        
        do {
            try fileManager.createDirectory(at: tempTempURL, withIntermediateDirectories: true, attributes: nil)
        } catch {
            return nil
        }
        
        return tempTempURL
    }

}


extension View {
    #if os(iOS)
    func exportFiles(homeState: HomeState, files: [File]) {
        DispatchQueue.global(qos: .userInitiated).async {
            var urls = []
            
            for file in files {
                if let url = ImportExportHelper.exportFilesToTempDir(homeState: homeState, file: file) {
                    urls.append(url)
                }
            }
            
            DispatchQueue.main.async {
                let activityVC = UIActivityViewController(activityItems: urls, applicationActivities: nil)
                
                if UIDevice.current.userInterfaceIdiom == .pad {
                    let thisViewVC = UIHostingController(rootView: self)
                    activityVC.popoverPresentationController?.sourceView = thisViewVC.view
                }
                
                UIApplication.shared.connectedScenes.flatMap {($0 as? UIWindowScene)?.windows ?? []}.first {$0.isKeyWindow}?.rootViewController?.present(activityVC, animated: true, completion: nil)
            }
        }
    }
    
    #else
    
    func exportFiles(homeState: HomeState, files: [File]) {
        guard let view = NSApp.keyWindow?.toolbar?.items.first?.view else {
            return
        }
        
        DispatchQueue.global(qos: .userInitiated).async {
            var urls = []
            
            for file in files {
                if let url = ImportExportHelper.exportFilesToTempDir(homeState: homeState, file: file) {
                    urls.append(url)
                }
            }

            DispatchQueue.main.async {
                NSSharingServicePicker(items: urls).show(relativeTo: .zero, of: view, preferredEdge: .minX)
            }
        }
    }
    
    #endif
}
