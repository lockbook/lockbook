import SwiftUI
import SwiftWorkspace

class FilesViewModel: ObservableObject {
    @Published var loaded = false
    
    @Published var files: [File] = []
    var idsToFiles: [UUID: File] = [:]
    var parents: [UUID: [File]] = [:]
    
    var error: String? = nil
    
    init(setLoaded: Bool = true) {
        loadFiles(setLoaded)
    }
    
    func loadFiles(_ setLoaded: Bool = true) {
        DispatchQueue.global(qos: .userInitiated).async {
            let res = MainState.lb.listMetadatas()
            DispatchQueue.main.async {
                switch res {
                case .success(let files):
                    self.idsToFiles = files.reduce(into: [:]) {a, b in
                        a[b.id] = b
                    }
                    self.parents = files.reduce(into: [:]) {
                        if $0[$1.parent] == nil {
                            $0[$1.parent] = []
                        }
                        
                        $0[$1.parent]!.append($1)
                    }
                    self.files = files
                    if setLoaded {
                        self.loaded = true
                    }
                case .failure(let err):
                    self.error = err.msg
                }
            }
        }
    }
}
