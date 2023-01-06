import Foundation
import SwiftUI
import SwiftLockbookCore

struct ShareFileSheet: View {
    
    @EnvironmentObject var settings: SettingsService
    @EnvironmentObject var sheets: SheetState
    @EnvironmentObject var sync: SyncService
    
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
                TextField("Username", text: $username)
                    .disableAutocorrection(true)
                    .textFieldStyle(RoundedBorderTextFieldStyle())
                
                Picker("Share mode", selection: $isWriteSelected) {
                    Text("Write").tag(true)
                    Text("Read").tag(false)
                }
                
                Button("Share") {
                    settings.shareFile(id: meta.id, username: username, isWrite: isWriteSelected)
                    presentationMode.wrappedValue.dismiss()
                    sync.sync()
                }
                    .buttonStyle(.borderedProminent)
                Spacer()
            }
                    .padding()
        }
    }
    
//    @ViewBuilder
//    var pendingShares: some View {}
}
