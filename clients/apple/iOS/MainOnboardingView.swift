import Foundation
import SwiftUI

struct OnboardingOneView: View {
    var body: some View {
        NavigationStack {
            VStack(alignment: .leading) {
                HStack {
                    Image(uiImage: UIImage(named: "logo")!)
                        .resizable()
                        .scaledToFit()
                        .frame(width: 75)
                    
                    Spacer()
                }
                
                Text("Lockbook")
                    .font(.largeTitle)
                    .fontWeight(.bold)
                    .padding(.leading)
                
                Text("The private note-taking platform.")
                    .font(.title2)
                    .padding(.leading)
                
                Spacer()
                
                NavigationLink(destination: {
                    OnboardingTwoView()
                }, label: {
                    Text("Get started")
                        .fontWeight(.semibold)
                        .frame(maxWidth: .infinity)
                        .frame(height: 30)
                })
                .buttonStyle(.borderedProminent)
                .padding(.bottom, 6)
                
                NavigationLink(destination: {
                    OnboardingTwoView()
                }, label: {
                    Text("I have an account")
                        .fontWeight(.semibold)
                        .frame(maxWidth: .infinity)
                        .frame(height: 30)
                })
                .buttonStyle(.bordered)
            }
            .padding(.top, 35)
            .padding(.horizontal)
        }
    }
}

struct OnboardingOneView_Previews: PreviewProvider {
    static var previews: some View {
        OnboardingOneView()
    }
}

struct OnboardingTwoView: View {
    @State var username: String = ""
    
    var body: some View {
        VStack(alignment: .leading) {
            Text("Create a username")
                .font(.title)
                .fontWeight(.bold)

            Text("Use letters **(A-Z)** and numbers **(0-9)**. Special characters aren’t allowed.")
                .padding(.top)
            
            Text("You cannot change your username later.")
                .padding(.top, 6)
            
            TextField("Username", text: $username)
                .padding(.top, 20)
            
            NavigationLink(destination: {
                OnboardingThreeView(username: username)
            }, label: {
                Text("Next")
                    .fontWeight(.semibold)
                    .frame(maxWidth: .infinity)
                    .frame(height: 30)
            })
            .buttonStyle(.borderedProminent)
            .disabled(username.isEmpty)
            .padding(.top, 30)
            
            Spacer()
        }
        .padding(.top, 35)
        .padding(.horizontal, 25)
    }
}

struct OnboardingTwoView_Previews: PreviewProvider {
    static var previews: some View {
        OnboardingTwoView()
    }
}

struct OnboardingThreeView: View {
    let username: String
    @State var storedSecurely = false
    
    var body: some View {
        VStack(alignment: .leading) {
            Text("This is your account key")
                .font(.title)
                .fontWeight(.bold)
            
            Text("It proves you’re you, and it is a secret. If you lose it, you can’t recover your account.")
                .padding(.top)
            
            Text("You can view you’re key again in the settings.")
                .padding(.top, 6)
                .padding(.bottom)
            
            HStack {
                Text("1. turkey\n2. era\n3. velvet\n4. detail\n5. prison\n6. income\n7. dose\n8. royal\n9. fever\n10. truly\n11. unique\n12. couple")
                    .padding(.leading, 30)
                Spacer()
                Text("13. party\n14. example\n15. piece\n16. art\n17. leaf\n18. follow\n19. rose\n20. access\n21. vacant\n22. gather\n23. wasp\n24. audit")
                    .padding(.trailing, 30)
            }
            .font(.system(.callout, design: .monospaced))
            .frame(maxWidth: .infinity)
            .padding()
            .background(RoundedRectangle(cornerRadius: 3).foregroundStyle(.gray).opacity(0.5))
            
            Spacer()
            
            HStack {
                Toggle(isOn: $storedSecurely, label: {
                    EmptyView()
                })
                .toggleStyle(iOSCheckboxToggleStyle())
                
                Text("It stored my account key in safe place.")
                    .font(.callout)
            }
            .padding(.top)
            .padding(.bottom)
            
            Button {
                print("nothing")
            } label: {
                Text("Copy compact key")
                    .fontWeight(.semibold)
                    .frame(maxWidth: .infinity)
                    .frame(height: 30)
            }
            .buttonStyle(.bordered)
            .padding(.bottom, 6)
            
            Button {
                print("nothing")
            } label: {
                Text("Next")
                    .fontWeight(.semibold)
                    .frame(maxWidth: .infinity)
                    .frame(height: 30)
            }
            .buttonStyle(.borderedProminent)
            .disabled(!storedSecurely)
        }
        .padding(.top, 35)
        .padding(.horizontal, 25)
    }
}

struct OnboardingThreeView_Previews: PreviewProvider {
    static var previews: some View {
        OnboardingThreeView(username: "smail")
    }
}

struct iOSCheckboxToggleStyle: ToggleStyle {
    func makeBody(configuration: Configuration) -> some View {
        Button(action: {
            configuration.isOn.toggle()
        }, label: {
            HStack {
                Image(systemName: configuration.isOn ? "checkmark.square" : "square")

                configuration.label
            }
        })
    }
}

struct ImportAccountView: View {
    @State var accountKey = ""
    
    var body: some View {
        VStack(alignment: .leading) {
            Text("Enter your key")
                .font(.title)
                .fontWeight(.bold)
            
            Text("Enter your phrase or private key, or scan your key QR from another device.")
                .padding(.top)
            
            Text("If you enter a phrase, please separate each word by a space or comma.")
                .padding(.top, 3)
                .padding(.bottom)
            
            HStack {
                TextField("Phrase or compact key", text: $accountKey)
                
                Button(action: {
                    print("do nothing")
                }, label: {
                    Image(systemName: "qrcode.viewfinder")
                        .font(.title)
                })
            }
            .padding(.top)
            .padding(.horizontal)
            
            HStack {
                Spacer()
                
                Button(action: {
                    print("do nothing")
                }, label: {
                    Text("Advanced")
                        .underline()
                })
                .padding(.trailing)
            }
            .padding(.top)
            
            Button {
                print("nothing")
            } label: {
                Text("Next")
                    .fontWeight(.semibold)
                    .frame(maxWidth: .infinity)
                    .frame(height: 30)
            }
            .buttonStyle(.borderedProminent)
            .padding(.top)
            .disabled(accountKey.isEmpty)
            
            Spacer()
        }
        .padding(.top, 35)
        .padding(.horizontal, 25)
    }
}

struct ImportAccountView_Previews: PreviewProvider {
    static var previews: some View {
        ImportAccountView()
    }
}
