import Foundation
import SwiftUI

struct UpgradeToPremium: View {
    
    @Environment(\.colorScheme) var colorScheme
    
    @State var showManageSub = false

    
    var body: some View {
        if showManageSub {
            Text("Purchase Premium")
                .font(.largeTitle)
                .bold()
                .multilineTextAlignment(.center)
                .padding(.top, 20)
            
            ManageSubscription()
        } else {
            VStack {
                Text("Upgrade to Lockbook\n Premium")
                    .font(.largeTitle)
                    .bold()
                    .multilineTextAlignment(.center)
                    .padding(.bottom, 50)
                    .padding(.top, 20)
                    .frame(height: 140)
                
                HStack {
                    Image(systemName: "cloud")
                        .foregroundColor(.blue)
                        .font(.system(size: 30))
                        .padding(.horizontal)
                    
                    VStack(alignment: .leading) {
                        Text("30 GB for $2.99/month,")
                            .bold()
                        
                        Text("Write notes on mac, iPhone, android, and linux. Lockbook will sync them across all your devices.")
                            .foregroundColor(.gray)
                            .lineLimit(2, reservesSpace: true)
                            .frame(width: 300)
                    }
                    
                    Spacer()
                }
                .padding(.bottom, 100)
                
                Button("Upgrade now", action: {
                    showManageSub = true
                })
                .frame(maxWidth: .infinity)
                .buttonStyle(.borderless)
                .padding(8)
                .background(Color.blue)
                .foregroundColor(.white)
                .cornerRadius(10)
                
            }
            .padding()
            .frame(width: 450)
        }
    }
}
