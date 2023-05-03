import Foundation
import SwiftUI

struct UpgradeToPremium: View {
    
    @Environment(\.colorScheme) var colorScheme
    @EnvironmentObject var settings: SettingsService
    
    var body: some View {
        VStack {
            Text("Upgrade to Lockbook\n Premium")
                .font(.largeTitle)
                .bold()
                .multilineTextAlignment(.center)
                .padding(.bottom, 50)
                .padding(.top, 20)
            
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
            

            Text("If you don't want to upgrade, you can \(Text("use Lockbook offline").foregroundColor(.blue)).")
            .font(.caption2)
            .foregroundColor(.gray)
            .padding(.bottom, 20)
            
            
            Button("Upgrade now", action: {
                
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
//        .background(RoundedRectangle(cornerRadius: 15).fill(colorScheme == .light ? .white : .black))
    }
}

struct UpgradeToPremium_Preview: PreviewProvider {
    static var previews: some View {
        UpgradeToPremium()
    }
}

