import Foundation
import SwiftUI
import SwiftWorkspace

class SelectedFilesState: ObservableObject {
    @Published var selectedFiles: Set<File>? = nil {
        didSet {
            if selectedFiles == nil {
                totalSelectedFiles = nil
            } else if selectedFiles?.isEmpty == true {
                totalSelectedFiles = []
            }
        }
    }
    @Published var totalSelectedFiles: Set<File>? = nil
    
    func addFileToSelection(file: File) {
        if selectedFiles?.contains(file) == true {
            return
        }
        
        selectedFiles?.insert(file)
        totalSelectedFiles?.insert(file)
        
        if file.type == .folder {
            var childrenToAdd = DI.files.childrenOf(file)
            
            while !childrenToAdd.isEmpty {
                var newChildren: [File] = []
                for child in childrenToAdd {
                    totalSelectedFiles?.insert(child)
                    selectedFiles?.remove(child)
                    if child.type == .folder {
                        newChildren.append(contentsOf: DI.files.childrenOf(child))
                    }
                }
                
                childrenToAdd = newChildren
            }
        }
    }
    
    func removeFileFromSelection(file: File) {
        if totalSelectedFiles?.contains(file) == false {
            return
        }
        
        selectedFiles?.remove(file)
        totalSelectedFiles?.remove(file)
        
        var before = file
        var current = DI.files.idsAndFiles[file.parent]
        
        if current?.id != current?.parent {
            while current != nil {
                if totalSelectedFiles?.contains(current!) == true {
                    selectedFiles?.remove(current!)
                    totalSelectedFiles?.remove(current!)
                    
                    for child in DI.files.childrenOf(current) {
                        if child != before {
                            totalSelectedFiles?.insert(child)
                            selectedFiles?.insert(child)
                        }
                    }
                    
                    let newCurrent = DI.files.idsAndFiles[current!.parent]
                    before = current!
                    current = newCurrent?.id == newCurrent?.parent ? nil : newCurrent
                } else {
                    current = nil
                }
            }
        }
        
        if file.type == .folder {
            var childrenToRemove = DI.files.childrenOf(file)
            
            while !childrenToRemove.isEmpty {
                var newChildren: [File] = []
                
                for child in childrenToRemove {
                    if (selectedFiles?.remove(child) == child || totalSelectedFiles?.remove(child) == child) && child.type == .folder {
                        newChildren.append(contentsOf: DI.files.childrenOf(child))
                    }
                }
                
                childrenToRemove = newChildren
            }
        }
    }

    
    func getConsolidatedSelectedFiles() -> [File] {
        var consSelectedFiles: [File] = []
        
        for file in selectedFiles ?? [] {
            var isUniq = true
            var parent = DI.files.idsAndFiles[file.parent]
            
            while parent != nil && parent?.id != parent?.parent {
                if selectedFiles?.contains(parent!) == true {
                    isUniq = false
                    break
                }
                
                parent = DI.files.idsAndFiles[parent!.parent]
            }
            
            if isUniq {
                consSelectedFiles.append(file)
            }
        }
        
        return consSelectedFiles
    }
}
