import Foundation
import SwiftLockbookCore
import SwiftUI

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
        
        if file.fileType == .Folder {
            var childrenToAdd = DI.files.childrenOf(file)
            
            while !childrenToAdd.isEmpty {
                var newChildren: [File] = []
                for child in childrenToAdd {
                    totalSelectedFiles?.insert(child)
                    if child.fileType == .Folder {
                        newChildren.append(contentsOf: DI.files.childrenOf(child))
                    }
                }
                
                childrenToAdd = newChildren
            }
            
            let children = DI.files.childrenOf(file)
            for child in children {
                selectedFiles?.remove(child)
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
                if selectedFiles?.contains(current!) == true {
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
        
        if file.fileType == .Folder {
            var childrenToRemove = DI.files.childrenOf(file)
            
            while !childrenToRemove.isEmpty {
                var newChildren: [File] = []
                
                for child in childrenToRemove {
                    if (selectedFiles?.remove(child) == child || totalSelectedFiles?.remove(child) == child) && child.fileType == .Folder {
                        newChildren.append(contentsOf: DI.files.childrenOf(child))
                    }
                }
                
                childrenToRemove = newChildren
            }
        }
    }

}
