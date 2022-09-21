//
//  ManageSubscription.swift
//  Lockbook
//
//  Created by Parth Mehrotra on 9/21/22.
//

import SwiftUI

struct ManageSubscription: View {
    var body: some View {
        VStack(alignment: .leading) {
            Text("Billing Screen")
        }.navigationTitle("Billing")
    }
}

struct ManageSubscription_Previews: PreviewProvider {
    static var previews: some View {
        NavigationView {
            ManageSubscription()
        }
    }
}
