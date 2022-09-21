//
//  ManageSubscription.swift
//  Lockbook
//
//  Created by Parth Mehrotra on 9/21/22.
//

import SwiftUI

struct ManageSubscription: View {
    
    @EnvironmentObject var settingsState: SettingsService
    
    var body: some View {
        VStack(alignment: .leading) {
            Text("Current Usage:")
            ColorProgressBar(value: settingsState.usageProgress)
            
            switch settingsState.tier {
            case .Trial: trial
            case .Premium: trial
            case .Unknown: trial
            }
            
        }.navigationTitle("Billing")
    }
    
    @ViewBuilder
    var trial: some View {
        Text("If you upgraded, your usage would be:")
        ColorProgressBar(value: settingsState.premiumProgress)
    }
}

struct ManageSubscription_Previews: PreviewProvider {
    static var previews: some View {
        NavigationView {
            ManageSubscription()
                .mockDI()
        }
    }
}
