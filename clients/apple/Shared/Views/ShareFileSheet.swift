import Foundation
import SwiftUI
import SwiftWorkspace

struct ShareFileSheet: View {
    let file: File
    
    @State var isWriteMode: Bool = true
    @State var username: String = ""
    @State var error: String = ""
    
    var readAccessUsers: [String] {
        file.shares.filter({ $0.mode == .read }).map({ $0.with })
    }
    var writeAccessUsers: [String] {
        file.shares.filter({ $0.mode == .write }).map({ $0.with })
    }
    
    @FocusState var focused: Bool
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 10) {
            HStack {
                Text("Share File")
                    .bold()
                
                Spacer()
            }
            
            TextField("Username", text: $username)
                .disableAutocorrection(true)
                .modifier(DisableAutoCapitalization())
                .textFieldStyle(.plain)
                .focused($focused)
                .onAppear {
                    focused = true
                }
                .onSubmit {
                    shareFile()
                }
            
            Picker("Flavor", selection: $isWriteMode) {
                Text("Write").tag(true)
                Text("Read").tag(false)
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            
            HStack {
                Text(error)
                    .foregroundStyle(.red)
                    .fontWeight(.bold)
                    .lineLimit(1, reservesSpace: true)
                
                Spacer()
            }
            
            Button {
                shareFile()
            } label: {
                Text("Share")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .disabled(username.isEmpty)
            
            HStack {
                Text("Share Access")
                    .bold()
                
                Spacer()
            }
            .padding(.top)
            
            HStack {
                Text(readAccessUsers.isEmpty ? "No users have read access." : "Read Access:")
                
                if !readAccessUsers.isEmpty {
                    ScrollView(.horizontal) {
                        HStack(spacing: 10) {
                            ForEach(readAccessUsers, id: \.self) { username in
                                Text(username)
                                    .padding(3)
                                    .modifier(CardBackground())
                            }
                        }
                        .padding(.horizontal)
                    }
                } else {
                    Spacer()
                }
            }
            .frame(height: 25)
            
            HStack {
                Text(writeAccessUsers.isEmpty ? "No users have write access." : "Write Access:")
                
                if !writeAccessUsers.isEmpty {
                    ScrollView(.horizontal) {
                        HStack(spacing: 10) {
                            ForEach(writeAccessUsers, id: \.self) { username in
                                Text(username)
                                    .padding(3)
                                    .modifier(CardBackground())
                            }
                        }
                        .padding(.horizontal)
                    }
                } else {
                    Spacer()
                }
            }
            .frame(height: 25)
        }
        .padding(.horizontal)
        .padding(.top, 3)
    }
    
    func shareFile() {
        let res = DI.core.shareFile(id: file.id, username: username, mode: isWriteMode ? .write : .read)
        
        switch res {
        case .success():
            DI.workspace.syncRequested = true
            dismiss()
        case .failure(let err):
            error = err.msg
        }
    }
}

#if os(iOS)
#Preview() {
    let file = File(id: UUID(), name: "", type: .document, parent: UUID(), lastModifiedBy: "", lastModified: 0, shares: [Share(by: "Smail", with: "Adam", mode: .write)])
    
    if UIDevice.current.userInterfaceIdiom == .pad {
        Rectangle()
            .foregroundStyle(.white)
            .modifier(FormSheetViewModifier(show: Binding.constant(true), sheetContent: {
                ShareFileSheet(file: file)
                    .padding(.bottom, 3)
                    .frame(width: 350, height: 260)
            }))
    } else {
        Rectangle()
            .foregroundStyle(.white)
            .sheet(isPresented: Binding.constant(true), content: {
                ShareFileSheet(file: file)
                    .presentationDetents([.height(200)])
                    .presentationDragIndicator(.visible)
            })
    }
}
#else
#Preview() {
    Rectangle()
        .foregroundStyle(.white)
        .sheet(isPresented: Binding.constant(true), content: {
            ShareFileSheet(file: File(id: UUID(), name: "", type: .document, parent: UUID(), lastModifiedBy: "", lastModified: 0, shares: [SwiftWorkspace.Share(by: "Smail", with: "Adam", mode: .write)]))
        })
}
#endif

struct CardBackground: ViewModifier {
    func body(content: Content) -> some View {
        content
            .background(
                RoundedRectangle(cornerRadius: 5)
                    .fill(.background)
                    .shadow(color: .black.opacity(0.2), radius: 4)
            )
            .padding(.vertical, 5)
    }
}
