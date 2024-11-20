import Foundation
import SwiftUI
import SwiftWorkspace

struct ShareFileSheet: View {
    let file: File
    
    @State var mode: ShareMode = .write
    @State var username: String = ""
    @State var error: String = ""
    
    @Environment(\.colorScheme) var colorScheme
    
    var readAccessUsers: [String] {
        file.shares.filter({ $0.mode == .read }).map({ $0.with })
    }
    var writeAccessUsers: [String] {
        file.shares.filter({ $0.mode == .write }).map({ $0.with })
    }
    
    var userCardBackground: Color {
        #if os(iOS)
        Color(UIColor.tertiarySystemBackground)
        #else
        colorScheme == .dark ? Color(nsColor: .windowBackgroundColor) : Color(nsColor: .controlBackgroundColor)
        #endif
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
            
            HStack {
                Text("File:")
                    .font(.callout)
                
                Text(file.name)
                    .font(.system(.callout, design: .monospaced))
                
                Spacer()
            }
            
            TextField("Username", text: $username)
                .disableAutocorrection(true)
                .modifier(DisableAutoCapitalization())
                .modifier(ShareFileTextField())
                .focused($focused)
                .onAppear {
                    focused = true
                }
                .onSubmit {
                    shareFile()
                }
            
            Picker("Flavor", selection: $mode) {
                Text("Write").tag(ShareMode.write)
                Text("Read").tag(ShareMode.read)
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
                                    .modifier(CardBackground(background: userCardBackground))
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
                                    .modifier(CardBackground(background: userCardBackground))
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
        let res = DI.core.shareFile(id: file.id, username: username, mode: mode)
        
        switch res {
        case .success():
            DI.workspace.syncRequested = true
            dismiss()
        case .failure(let err):
            error = err.msg
        }
    }
}

struct CardBackground: ViewModifier {
    let background: Color
    
    func body(content: Content) -> some View {
        content
            .background(
                RoundedRectangle(cornerRadius: 5)
                    .fill(background)
                    .shadow(color: .black.opacity(0.2), radius: 4)
            )
            .padding(.vertical, 5)
    }
}

struct ShareFileTextField: ViewModifier {
    func body(content: Content) -> some View {
        #if os(iOS)
        content
            .textFieldStyle(.roundedBorder)
        #else
        content
            .textFieldStyle(.plain)
        #endif
    }
}
