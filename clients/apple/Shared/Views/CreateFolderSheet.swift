import Foundation
import SwiftUI
import SwiftLockbookCore

struct CreateFolderSheet: View {
    let info: CreatingFolderInfo
    
    @State var name: String = ""
    @State var error: String? = nil
    @State var sheetHeight: CGFloat = 0
    
    @Environment(\.dismiss) private var dismiss
    @FocusState private var isFocused: Bool
    
    var body: some View {
        VStack(spacing: 10) {
            HStack {
                Text("New folder")
                    .bold()
                
                Spacer()
            }
            
            LabeledContent {
                Text(info.parentPath)
                    .lineLimit(2)
                    .font(.system(.callout, design: .monospaced))
            } label: {
                Text("Parent:")
                    .font(.callout)
            }
            
            TextField("Folder name", text: $name, onCommit: {
                createFolder()
            })
            .textFieldStyle(.roundedBorder)
            .focused($isFocused)
            .onAppear {
                isFocused = true
            }
            
            if let error = error {
                HStack {
                    Text(error)
                        .foregroundStyle(.red)
                        .fontWeight(.bold)
                        .lineLimit(2, reservesSpace: false)
                    
                    Spacer()
                }
            }
                        
            Button {
                createFolder()
            } label: {
                Text("Create")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)

        }
        .padding(.horizontal)
        .padding(.top)
        .modifier(ReadHeightModifier())
        .onPreferenceChange(HeightPreferenceKey.self) { height in
            if let height {
                self.sheetHeight = height
            }
        }
        .presentationDetents([.height(self.sheetHeight)])
        .presentationDragIndicator(.visible)
    }
    
    func createFolder() {
        let res = DI.files.createFolderSync(name: name, maybeParent: info.maybeParent)
        
        switch res {
        case .some(let errMsg):
            error = errMsg
        case .none:
            dismiss()
        }
    }
}

struct CreateFolderSheet_Previews: PreviewProvider {
    static var previews: some View {
        Rectangle()
            .foregroundStyle(.white)
            .sheet(isPresented: Binding.constant(true), content: {
                CreateFolderSheet(info: CreatingFolderInfo(parentPath: "Apple", maybeParent: nil))
                    .presentationDetents([.height(150)])
                    .presentationDragIndicator(.visible)
            })
    }
}
