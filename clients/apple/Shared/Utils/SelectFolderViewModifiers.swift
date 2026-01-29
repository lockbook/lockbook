import SwiftUI

struct SelectFolderButtonViewModifier: ViewModifier {
    func body(content: Content) -> some View {
        content.buttonStyle(.plain)
    }
}

struct SelectedItemViewModifier: ViewModifier {
    let isSelected: Bool
    
    init(item: String, selected: String) {
        isSelected = item == selected
    }
    
    func body(content: Content) -> some View {
        if isSelected {
            content.background(RoundedRectangle(cornerRadius: 5).fill(Color.gray.opacity(0.2)).padding(.horizontal, 10))
        } else {
            content
        }
    }
}

extension View {
    func selectFolderButton() -> some View {
        modifier(SelectFolderButtonViewModifier())
    }
    
    func selectedItem(item: String, selected: String) -> some View {
        modifier(SelectedItemViewModifier(item: item, selected: selected))
    }
}

