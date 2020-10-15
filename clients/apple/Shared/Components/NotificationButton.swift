import SwiftUI

struct NotificationButton: View {
    let action: () -> Result<Void, Error>
    let label: Label<Text, Image>
    let successLabel: Label<Text, Image>
    let failureLabel: Label<Text, Image>
    @State var success: Bool?
    
    var body: some View {
        HStack {
            Button(action: handler, label: { label })
                .disabled(success != nil)
            success.map { res in
                (res ? successLabel : failureLabel)
                    .foregroundColor(res ? .green : .red)
                    .opacity(0.8)
                    .onAppear { DispatchQueue.main.asyncAfter(deadline: .now() + 4, execute: dismiss) }
                    .onTapGesture { dismiss() }
            }
        }
        
    }
    
    func handler() {
        withAnimation {
            switch action() {
            case .success(_):
                success = true
            case .failure(_):
                success = false
            }
        }
    }
    
    func dismiss() {
        withAnimation(.linear) {
            success = nil
        }
    }
}

struct NotificationButton_Previews: PreviewProvider {
    static var previews: some View {
        NotificationButton(
            action: { Result.success(()) },
            label: Label("Fuck", systemImage: "doc"),
            successLabel: Label("Success", systemImage: "checkmark.square"),
            failureLabel: Label("Failure", systemImage: "exclamationmark.square")
        )
    }
}
