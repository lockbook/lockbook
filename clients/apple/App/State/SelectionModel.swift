import Foundation
import SwiftUI
import SwiftWorkspace

#if os(macOS)
    import AppKit
#endif

@Observable class SelectionModel {
    var selecting = false
    var selectedIds: Set<UUID> = []

    @ObservationIgnored private var anchor: UUID? = nil

    func begin(with id: UUID) {
        selecting = true
        selectedIds.insert(id)
        anchor = id
    }

    func toggle(_ id: UUID) {
        selecting = true

        if selectedIds.remove(id) == nil {
            selectedIds.insert(id)
            anchor = id
        }
    }

    func extend(to id: UUID, in orderedIds: [UUID]) {
        selecting = true

        guard let anchor,
              let anchorIndex = orderedIds.firstIndex(of: anchor),
              let index = orderedIds.firstIndex(of: id)
        else {
            selectedIds.insert(id)
            anchor = id
            return
        }

        for selected in orderedIds[min(anchorIndex, index) ... max(anchorIndex, index)] {
            selectedIds.insert(selected)
        }
    }

    func end() {
        selecting = false
        selectedIds = []
        anchor = nil
    }

    func handleTap(id: UUID, orderedIds: () -> [UUID], open: () -> Void) {
        #if os(macOS)
            let modifiers = NSEvent.modifierFlags

            if modifiers.contains(.shift) {
                withAnimation {
                    extend(to: id, in: orderedIds())
                }
                return
            }

            if modifiers.contains(.command) {
                withAnimation {
                    toggle(id)
                }
                return
            }
        #endif

        if selecting {
            withAnimation {
                toggle(id)
            }
        } else {
            open()
        }
    }

    func selectedFiles(in filesModel: FilesModel) -> [File] {
        selectedIds.compactMap { filesModel.idsToFiles[$0] }
            .filter { file in
                var current = file

                while let parent = filesModel.idsToFiles[current.parent],
                      !parent.isRoot, parent.id != current.id
                {
                    if selectedIds.contains(parent.id) {
                        return false
                    }
                    current = parent
                }

                return true
            }
    }
}
