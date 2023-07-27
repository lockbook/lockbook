import Foundation
import SwiftUI
import SwiftLockbookCore

struct ShareFileSheet: View {
    
    @EnvironmentObject var share: ShareService
    @EnvironmentObject var sheets: SheetState
    @EnvironmentObject var sync: SyncService
    @EnvironmentObject var fileService: FileService
    
    @Environment(\.presentationMode) var presentationMode
    
    @State var isWriteSelected: Bool = true
    @State var username: String = ""
    
    var body: some View {
        if let meta = sheets.sharingFileInfo {
            VStack(alignment: .leading, spacing: 15) {
                HStack(alignment: .center) {
                    Text("Sharing: \(meta.name)")
                        .bold()
                        .font(.title)
                    
                    Spacer()
                    
                    Button(action: { presentationMode.wrappedValue.dismiss() }) {
                        Image(systemName: "xmark.circle.fill")
                            .foregroundColor(.gray)
                            .imageScale(.large)
                            .frame(width: 50, height: 50, alignment: .center)
                    }
                }
                
                #if os(iOS)
                TextField("Username", text: $username)
                    .disableAutocorrection(true)
                    .textInputAutocapitalization(.never)
                    .textFieldStyle(RoundedBorderTextFieldStyle())
                #else
                TextField("Username", text: $username)
                    .disableAutocorrection(true)
                    .textFieldStyle(RoundedBorderTextFieldStyle())
                #endif
                
                #if os(macOS)
                shareMode
                    .pickerStyle(.radioGroup)
                #elseif os(iOS)
                shareMode
                #endif
                
                Button("Share") {
                    share.shareFile(id: meta.id, username: username, isWrite: isWriteSelected)
                    sync.sync()

                    presentationMode.wrappedValue.dismiss()
                }
                .buttonStyle(.borderedProminent)
                
                Spacer()
                
                Text("Write Access:")
                if let shareInfo = share.shareInfos[meta] {
                    ScrollView(.horizontal) {
                        HStack(spacing: 40) {
                            ForEach(shareInfo.writeAccessUsers, id: \.self) { username in
                                SharedUserCell(username: username)
                            }
                        }
                        .padding(.horizontal)
                    }
                }
                
                
                Text("Read Access:")
                if let shareInfo = share.shareInfos[meta] {
                    ScrollView(.horizontal) {
                        HStack(spacing: 40) {
                            ForEach(shareInfo.readAccessUsers, id: \.self) { username in
                                SharedUserCell(username: username)
                            }
                        }
                        .padding(.horizontal)
                    }
                }
            }
            .frameForMacOS()
            .padding()
            .onAppear {
                share.calculateShareInfo(id: meta.id)
            }
        }
    }
    
    @ViewBuilder
    var shareMode: some View {
        Picker("Share mode", selection: $isWriteSelected) {
            Text("Write").tag(true)
            Text("Read").tag(false)
        }
    }
}
    
struct SharedUserCell: View {
    let username: String
    
    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "person.circle")
                .foregroundColor(.blue)
                
            Text(username)
                .font(.body)
                .frame(width: 50)
        }
            .contentShape(Rectangle())
            .frame(maxWidth: 50)
    }
}

let usernames = ["smail", "parth", "travis", "steve"]

struct SharedUserCell_Previews: PreviewProvider {
    static var previews: some View {
        ScrollView(.horizontal) {
            HStack(spacing: 40) {
                ForEach(usernames, id: \.self) {username in
                    SharedUserCell(username: username)
                }
            }
            .padding(.horizontal)
        }
    }
}
