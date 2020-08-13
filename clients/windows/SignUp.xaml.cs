using System;
using Windows.UI.Popups;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;

namespace lockbook {

    public sealed partial class SignUp : Page {
        public SignUp() {
            InitializeComponent();
        }

        private async void ImportAccount(object sender, RoutedEventArgs e) {
            System.Diagnostics.Debug.WriteLine("Import");
        }

        private async void CreateAccount(object sender, RoutedEventArgs e) {
            createAccount.IsEnabled = false;
            progressGroup.Visibility = Visibility.Visible;
            progressRing.Visibility = Visibility.Visible;
            progressRing.IsActive = true;
            error.Visibility = Visibility.Collapsed;
            var createAccountResult = await CoreService.CreateAccount(username.Text);

            switch (createAccountResult) {
                case Core.CreateAccount.Success:
                    Frame.Navigate(typeof(FileExplorer));
                    break;
                case Core.CreateAccount.UnexpectedError ohNo:
                    await new MessageDialog(ohNo.errorMessage, "Unexpected Error!").ShowAsync();
                    break;
                case Core.CreateAccount.ExpectedError exptectedError:
                    switch (exptectedError.error) {
                        case Core.CreateAccount.PossibleErrors.InvalidUsername:
                            error.Text = "Invalid username!";
                            error.Visibility = Visibility.Visible;
                            break;
                        case Core.CreateAccount.PossibleErrors.UsernameTaken:
                            error.Text = "Username taken!";
                            error.Visibility = Visibility.Visible;
                            break;
                        case Core.CreateAccount.PossibleErrors.CouldNotReachServer:
                            error.Text = "Could not reach server!";
                            error.Visibility = Visibility.Visible;
                            break;
                        case Core.CreateAccount.PossibleErrors.AccountExistsAlready:
                            error.Text = "An account exists already!";
                            error.Visibility = Visibility.Visible;
                            break;
                    };
                    break;
            }

            createAccount.IsEnabled = true;
            progressGroup.Visibility = Visibility.Collapsed;
            progressRing.Visibility = Visibility.Collapsed;
            progressRing.IsActive = false;
        }
    }
}
