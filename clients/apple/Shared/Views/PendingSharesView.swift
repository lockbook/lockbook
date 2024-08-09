import Foundation
import SwiftUI
import SwiftLockbookCore

struct PendingSharesView: View {
    
    @EnvironmentObject var sheets: SheetState
    @EnvironmentObject var share: ShareService
    
    var body: some View {
        VStack {
            if let shares = share.pendingShares {
                if shares.isEmpty {
                    noPendingShares
                } else {
                    pendingShares
                }
            } else {
                ProgressView()
            }
        }
        .onAppear {
            share.calculatePendingShares()
        }
    }
    
    @ViewBuilder
    var pendingShares: some View {
        Group {
            List(share.pendingShares!.sorted { meta1, meta2 in
                meta1 > meta2
            }) { meta in
                SharedFileCell(meta: meta)
            }
            .listStyle(InsetListStyle())
            
            Spacer()
        }
        .navigationTitle("Pending Shares")
    }
    
    @ViewBuilder
    var noPendingShares: some View {
        Group {
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
            Image(systemName: FileService.metaToSystemImage(meta: meta))
                .foregroundColor(meta.fileType == .Folder ? .blue : .secondary)
                .imageScale(.large)
                
            Text(meta.name)
                .font(.title3)
                
            Spacer()
            
            Button {
                sheets.movingInfo = .AcceptShare((meta.name, meta.id))
            } label: {
                Image(systemName: "plus.circle")
                    .imageScale(.medium)
                    .foregroundColor(.blue)
            }
            
            Button {
                showRejectConfirmation = true
            } label: {
                Image(systemName: "minus.circle")
                    .imageScale(.medium)
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

