import SwiftUI
import SwiftWorkspace
import Combine

class SuggestedDocsViewModel: ObservableObject {
    @Published var suggestedDocs: [SuggestedDocInfo]? = nil
    
    var filesModel: FilesViewModel
    
    var cancellables: Set<AnyCancellable> = []
    
    init(filesModel: FilesViewModel) {
        self.filesModel = filesModel
        
        filesModel.$files.sink { [weak self] files in
            self?.loadSuggestedDocs()
        }
        .store(in: &cancellables)
    }
    
    func loadSuggestedDocs() {
        DispatchQueue.global(qos: .userInitiated).async {
            let res = AppState.lb.suggestedDocs()
            
            DispatchQueue.main.async {
                switch res {
                case .success(let ids):
                    let files = ids.compactMap({ self.filesModel.idsToFiles[$0] })
                    
                    self.suggestedDocs = files.compactMap({ file in
                        guard let parent = self.filesModel.idsToFiles[file.parent] else {
                            return .none
                        }
                                                
                        return .some(SuggestedDocInfo(
                            name: file.name,
                            id: file.id,
                            parentName: parent.name,
                            lastModified: AppState.lb.getTimestampHumanString(timestamp: Int64(file.lastModified))
                        ))
                    })
                case .failure(let error):
                    print("ignored for now")
                }
            }
            
        }
    }
}

struct SuggestedDocInfo: Identifiable {
    let name: String
    let id: UUID
    let parentName: String
    let lastModified: String
}
