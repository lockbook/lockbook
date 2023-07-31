import Foundation
import SwiftUI
import SwiftLockbookCore

struct PendingSharesView: View {
    
    @EnvironmentObject var sheets: SheetState
    @EnvironmentObject var share: ShareService
    
    var body: some View {
        if share.pendingShares.isEmpty {
            noPendingShares
        } else {
            pendingShares
        }
    }
    
    @ViewBuilder
    var pendingShares: some View {
        VStack {
            List(share.pendingShares.sorted { meta1, meta2 in
                meta1 > meta2
            }) { meta in
                SharedFileCell(meta: meta)
            }
            
            Spacer()
        }
        .background(.clear)
        .navigationTitle("Pending Shares")
        .sheet(isPresented: $sheets.acceptingShare, content: AcceptShareSheet.init)
    }
    
    @ViewBuilder
    var noPendingShares: some View {
        VStack {
            Spacer()
            Image(systemName: "person.2.slash")
                .padding(.vertical, 5)
                .imageScale(.large)
            Text("You have no pending shares.")
            Spacer()
        }
        .navigationTitle("Pending Shares")
    }
}

struct SharedFileCell: View {
    @EnvironmentObject var sheets: SheetState
    @EnvironmentObject var share: ShareService
    
    let meta: File
    
    @State var showRejectConfirmation = false

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: meta.fileType == .Folder ? "folder" : "doc")
                .foregroundColor(meta.fileType == .Folder ? .blue : .secondary)
                
            Text(meta.name)
                .font(.title3)
                
            Spacer()
            
            Button {
                sheets.acceptingShareInfo = meta
            } label: {
                Image(systemName: "plus.circle")
                    .imageScale(.large)
                    .foregroundColor(.blue)
            }
            
            Button {
                showRejectConfirmation = true
            } label: {
                Image(systemName: "minus.circle")
                    .imageScale(.large)
                    .foregroundColor(.red)
            }
        }
                .padding(.vertical, 7)
                .contentShape(Rectangle())
                .confirmationDialog("Are you sure you want to reject \(meta.name)", isPresented: $showRejectConfirmation) {
                    Button("Reject \(meta.name)", role: .destructive) {
                        share.rejectShare(id: meta.id)
                    }
                }

    }
}

