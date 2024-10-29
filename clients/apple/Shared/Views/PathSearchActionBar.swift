import SwiftUI
#if os(iOS)
import UIKit
#endif

struct PathSearchActionBar: View {
    @State var text: String = ""
    
    @EnvironmentObject var search: SearchService
    
    #if os(macOS)
    @FocusState var focused: Bool
    #endif
    
    var body: some View {
        Group {
            Rectangle()
                .onTapGesture {
                    search.endSearch(isPathAndContentSearch: false)
                }
                .foregroundColor(.gray.opacity(0.01))
            
            GeometryReader { geometry in
                VStack {
                    VStack {
                        HStack {
                            Image(systemName: "magnifyingglass")
                            
                            #if os(iOS)
                            PathSearchTextFieldWrapper()
                                .frame(height: 30)
                            #else
                            PathSearchTextFieldWrapper()
                                .focused($focused)
                                .onAppear {
                                    focused = true
                                }
                            #endif
                            
                            if search.isPathSearchInProgress {
                                #if os(iOS)
                                ProgressView()
                                    .frame(width: 20, height: 20)
                                #else
                                ProgressView()
                                    .scaleEffect(0.5)
                                    .frame(width: 20, height: 20)
                                #endif
                            }
                        }
                        
                        if !search.pathSearchResults.isEmpty {
                            Divider()
                                .padding(.top)
                            
                            ScrollViewReader { scrollHelper in
                                ScrollView {
                                    ForEach(search.pathSearchResults) { result in
                                        if case .PathMatch(_, _, let name, let path, let matchedIndices, _) = result {
                                            let index = search.pathSearchResults.firstIndex(where: { $0.id == result.id }) ?? 0
                
                                            SearchResultCellView(name: name, path: path, matchedIndices: matchedIndices, index: index, selected: search.pathSearchSelected)
                                        }
                                    }
                                    .scrollIndicators(.visible)
                                    .padding(.horizontal)
                                }
                                .onChange(of: search.pathSearchSelected) { newValue in
                                    withAnimation {
                                        if newValue < search.pathSearchResults.count {
                                            let item = search.pathSearchResults[newValue]
                                            
                                            scrollHelper.scrollTo(item.id, anchor: .center)
                                        }
                                    }
                                }
                            }
                            .frame(maxHeight: 500)
                        } else if !search.isPathSearchInProgress && !search.pathSearchQuery.isEmpty {
                            Text("No results.")
                               .font(.headline)
                               .foregroundColor(.gray)
                               .fontWeight(.bold)
                               .padding()
                        }
                    }
                    .padding()
                    .background(
                        RoundedRectangle(cornerSize: CGSize(width: 20, height: 20))
                            .foregroundColor({
                                #if os(iOS)
                                Color(UIColor.secondarySystemBackground)
                                #else
                                Color(nsColor: .windowBackgroundColor)
                                #endif
                            }())
                            .shadow(radius: 10)
                    )
                    .frame(width: 500)
                }
                .padding(.top, geometry.size.height / 4.5)
                .padding(.leading, (geometry.size.width / 2) - 250)
                .padding(.bottom, 100)
            }
        }
    }
}

#if os(iOS)

class PathSearchTextField: UITextField {
    override var keyCommands: [UIKeyCommand]? {
        let selectedUp = UIKeyCommand(input: UIKeyCommand.inputUpArrow, modifierFlags: [], action: #selector(moveSelectedUp))
        let selectedDown = UIKeyCommand(input: UIKeyCommand.inputDownArrow, modifierFlags: [], action: #selector(moveSelectedDown))
        let exitPathSearch = UIKeyCommand(input: UIKeyCommand.inputEscape, modifierFlags: [], action: #selector(exitPathSearch))
        
        selectedUp.wantsPriorityOverSystemBehavior = true
        selectedDown.wantsPriorityOverSystemBehavior = true
        exitPathSearch.wantsPriorityOverSystemBehavior = true
        
        var shortcuts = [
            selectedUp,
            selectedDown,
            exitPathSearch
        ]
        
        let selectIndex1 = UIKeyCommand(input: "1", modifierFlags: [.command], action: #selector(openSelected1))
        let selectIndex2 = UIKeyCommand(input: "2", modifierFlags: [.command], action: #selector(openSelected2))
        let selectIndex3 = UIKeyCommand(input: "3", modifierFlags: [.command], action: #selector(openSelected3))
        let selectIndex4 = UIKeyCommand(input: "4", modifierFlags: [.command], action: #selector(openSelected4))
        let selectIndex5 = UIKeyCommand(input: "5", modifierFlags: [.command], action: #selector(openSelected5))
        let selectIndex6 = UIKeyCommand(input: "6", modifierFlags: [.command], action: #selector(openSelected6))
        let selectIndex7 = UIKeyCommand(input: "7", modifierFlags: [.command], action: #selector(openSelected7))
        let selectIndex8 = UIKeyCommand(input: "8", modifierFlags: [.command], action: #selector(openSelected8))
        let selectIndex9 = UIKeyCommand(input: "9", modifierFlags: [.command], action: #selector(openSelected9))
        
        selectIndex1.wantsPriorityOverSystemBehavior = true
        selectIndex2.wantsPriorityOverSystemBehavior = true
        selectIndex3.wantsPriorityOverSystemBehavior = true
        selectIndex4.wantsPriorityOverSystemBehavior = true
        selectIndex5.wantsPriorityOverSystemBehavior = true
        selectIndex6.wantsPriorityOverSystemBehavior = true
        selectIndex7.wantsPriorityOverSystemBehavior = true
        selectIndex8.wantsPriorityOverSystemBehavior = true
        selectIndex9.wantsPriorityOverSystemBehavior = true
        
        shortcuts.append(selectIndex1)
        shortcuts.append(selectIndex2)
        shortcuts.append(selectIndex3)
        shortcuts.append(selectIndex4)
        shortcuts.append(selectIndex5)
        shortcuts.append(selectIndex6)
        shortcuts.append(selectIndex7)
        shortcuts.append(selectIndex8)
        shortcuts.append(selectIndex9)
        
        return shortcuts
    }
    
    @objc func moveSelectedUp() {
        DI.search.selectPreviousPath()
    }
    
    @objc func moveSelectedDown() {
        DI.search.selectNextPath()
    }
    
    @objc func exitPathSearch() {
        DI.search.endSearch(isPathAndContentSearch: false)
    }
    
    @objc func openSelected1() {
        DI.search.openPathAtIndex(index: 0)
    }
    
    @objc func openSelected2() {
        DI.search.openPathAtIndex(index: 1)
    }
    
    @objc func openSelected3() {
        DI.search.openPathAtIndex(index: 2)
    }
    
    @objc func openSelected4() {
        DI.search.openPathAtIndex(index: 3)
    }
    
    @objc func openSelected5() {
        DI.search.openPathAtIndex(index: 4)
    }
    
    @objc func openSelected6() {
        DI.search.openPathAtIndex(index: 5)
    }
    
    @objc func openSelected7() {
        DI.search.openPathAtIndex(index: 6)
    }
    
    @objc func openSelected8() {
        DI.search.openPathAtIndex(index: 7)
    }
    
    @objc func openSelected9() {
        DI.search.openPathAtIndex(index: 8)
    }
}

public struct PathSearchTextFieldWrapper: UIViewRepresentable {
    @State var text: String = ""

    public func makeUIView(context: Context) -> UITextField {
        let textField = PathSearchTextField()
        textField.delegate = context.coordinator
        textField.becomeFirstResponder()
        textField.autocapitalizationType = .none
        textField.autocorrectionType = .no

        return textField
    }

    public func updateUIView(_ uiView: UITextField, context: Context) {
        uiView.text = text
        
        DispatchQueue.main.async {
            if DI.search.isPathSearching {
                DI.search.search(query: text, isPathAndContentSearch: false)
            }
        }
        
    }

    public func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }

    public class Coordinator: NSObject, UITextFieldDelegate {
        var parent: PathSearchTextFieldWrapper

        public init(_ parent: PathSearchTextFieldWrapper) {
            self.parent = parent
        }

        public func textFieldDidChangeSelection(_ textField: UITextField) {
            parent.text = textField.text ?? ""
        }

        public func textFieldShouldReturn(_ textField: UITextField) -> Bool {
            DI.search.openPathAtIndex(index: DI.search.pathSearchSelected)
            
            return false
        }
    }
}

#else

public class PathSearchTextField: NSTextField {
    public override func performKeyEquivalent(with event: NSEvent) -> Bool {
        switch event.keyCode {
        case 126: // up arrow
            DI.search.selectPreviousPath()
            return true
        case 125: // down arrow
            DI.search.selectNextPath()
            return true
        case 36: // return
            DI.search.openPathAtIndex(index: DI.search.pathSearchSelected)
            return true
        default:
            if event.modifierFlags.contains(.command) { // command + num (1-9)
                if event.keyCode == 18  {
                    DI.search.openPathAtIndex(index: 0)
                } else if event.keyCode == 19 {
                    DI.search.openPathAtIndex(index: 1)
                } else if event.keyCode == 20 {
                    DI.search.openPathAtIndex(index: 2)
                } else if event.keyCode == 21 {
                    DI.search.openPathAtIndex(index: 3)
                } else if event.keyCode == 23 {
                    DI.search.openPathAtIndex(index: 4)
                } else if event.keyCode == 22 {
                    DI.search.openPathAtIndex(index: 5)
                } else if event.keyCode == 26 {
                    DI.search.openPathAtIndex(index: 6)
                } else if event.keyCode == 28 {
                    DI.search.openPathAtIndex(index: 7)
                } else if event.keyCode == 25 {
                    DI.search.openPathAtIndex(index: 8)
                } else {
                    return super.performKeyEquivalent(with: event)
                }
                
                return true
            }
            
            
            return super.performKeyEquivalent(with: event)
        }
    }
    
    public override func cancelOperation(_ sender: Any?) {
        DI.search.endSearch(isPathAndContentSearch: false)
    }
}

public struct PathSearchTextFieldWrapper: NSViewRepresentable {
    let textField = PathSearchTextField()
            
    public func makeNSView(context: NSViewRepresentableContext<PathSearchTextFieldWrapper>) -> PathSearchTextField {
        textField.isBordered = false
        textField.focusRingType = .none
        textField.delegate = context.coordinator
        textField.font = .systemFont(ofSize: 15)
        textField.backgroundColor = nil
        
        return textField
    }
    
    public func updateNSView(_ nsView: PathSearchTextField, context: NSViewRepresentableContext<PathSearchTextFieldWrapper>) {
        textField.becomeFirstResponder()
    }
    
    public func makeCoordinator() -> PathSearchTextFieldDelegate {
        PathSearchTextFieldDelegate(self)
    }
    
    public class PathSearchTextFieldDelegate: NSObject, NSTextFieldDelegate {
        var parent: PathSearchTextFieldWrapper

        public init(_ parent: PathSearchTextFieldWrapper) {
            self.parent = parent
        }

        public func controlTextDidChange(_ obj: Notification) {
            if let textField = obj.object as? NSTextField {
                DispatchQueue.main.async {
                    if DI.search.isPathSearching {
                        DI.search.search(query: textField.stringValue, isPathAndContentSearch: false)
                    }
                }
            }
        }
    }
}

#endif

struct SearchResultCellView: View {
    let name: String
    let path: String
    let matchedIndices: [Int]
    
    let index: Int
    let selected: Int
    
    @State var pathModified: Text = Text("")
    @State var nameModified: Text = Text("")
    
    var body: some View {
        Button(action: {
            DI.search.pathSearchSelected = index
            DI.search.openPathAtIndex(index: index)
        }, label: {
            HStack {
                Image(systemName: FileService.docExtToSystemImage(name: name))
                    .resizable()
                    .frame(width: 20, height: 25)
                    .padding(.horizontal, 10)
                    .foregroundColor(index == selected ? .white : .primary)
                
                VStack(alignment: .leading) {
                    nameModified
                        .font(.system(size: 16))
                        .multilineTextAlignment(.leading)
                        .foregroundColor(index == selected ? .white : .primary)
                    
                    pathModified
                        .multilineTextAlignment(.leading)
                        .foregroundColor(index == selected ? .white : .gray)
                }
                
                Spacer()
                
                if index == selected {
                    Text("↩")
                        .padding(.horizontal)
                        .foregroundColor(index == selected ? .white : .gray)
                } else if index < 9 {
                    Text("⌘\(index + 1)")
                        .padding(.horizontal)
                        .foregroundColor(index == selected ? .white : .gray)
                }
            }
            .frame(height: 40)
            .padding(EdgeInsets(top: 4, leading: 0, bottom: 4, trailing: 0))
            .contentShape(Rectangle())
            .onAppear {
                underlineMatchedSegments()
            }
            .background(index == selected ? RoundedRectangle(cornerSize: CGSize(width: 5, height: 5)).foregroundColor(.blue.opacity(0.8)) : RoundedRectangle(cornerSize: CGSize(width: 5, height: 5)).foregroundColor(.clear))
        })
    }
    
    func underlineMatchedSegments() {
        let matchedIndicesHash = Set(matchedIndices)
        
        var pathOffset = 1;
        
        if(path.count - 1 > 0) {
            pathModified = Text("")
            
            for index in 0...path.count - 1 {
                let correctIndex = String.Index(utf16Offset: index, in: path)
                let newPart = Text(path[correctIndex...correctIndex])
                
                if(matchedIndicesHash.contains(index + 1)) {
                    pathModified = pathModified + newPart.bold()
                } else {
                    pathModified = pathModified + newPart
                }
            }
            
            pathOffset = 2
        }
                
        if(name.count - 1 > 0) {
            nameModified = Text("")
            for index in 0...name.count - 1 {
                let correctIndex = String.Index(utf16Offset: index, in: name)
                let newPart = Text(name[correctIndex...correctIndex])
                
                if(matchedIndicesHash.contains(index + path.count + pathOffset)) {
                    nameModified = nameModified + newPart.bold()
                } else {
                    nameModified = nameModified + newPart
                }
            }
        }
    }
}
