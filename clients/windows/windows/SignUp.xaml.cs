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
            createAccount.IsEnabled = false;
            importAccountButton.IsEnabled = false;
            importProgressGroup.Visibility = Visibility.Visible;
            ImportProgressRing.Visibility = Visibility.Visible;
            ImportProgressRing.IsActive = true;
            importError.Visibility = Visibility.Collapsed;

            Core.ImportAccount.Result importAccountResult = await App.CoreService.ImportAccount(accountStringTextBox.Text);

            switch (importAccountResult) {
                case Core.ImportAccount.Success:
                    break;
                case Core.ImportAccount.UnexpectedError ohNo:
                    await new MessageDialog(ohNo.ErrorMessage, "Unexpected Error!").ShowAsync();
                    break;
                case Core.ImportAccount.ExpectedError expectedError:
                    switch (expectedError.Error) {
                        case Core.ImportAccount.PossibleErrors.AccountDoesNotExist:
                            importError.Text = "That account does not exist on this server!";
                            newAccountError.Visibility = Visibility.Visible;
                            break;
                        case Core.ImportAccount.PossibleErrors.AccountExistsAlready:
                            importError.Text = "An account exists already, clear your app data to import another account!";
                            newAccountError.Visibility = Visibility.Visible;
                            break;
                        case Core.ImportAccount.PossibleErrors.AccountStringCorrupted:
                            importError.Text = "This account string is corrupt!";
                            newAccountError.Visibility = Visibility.Visible;
                            break;
                        case Core.ImportAccount.PossibleErrors.CouldNotReachServer:
                            importError.Text = "Could not reach the server!";
                            newAccountError.Visibility = Visibility.Visible;
                            break;
                        case Core.ImportAccount.PossibleErrors.UsernamePKMismatch:
                            importError.Text = "This username does not correspond to the public key in this account_string!";
                            newAccountError.Visibility = Visibility.Visible;
                            break;
                    };
                    break;
            }

            var syncResult = await App.CoreService.SyncAll();
            switch (syncResult) {
                case Core.SyncAll.Success:
                    Frame.Navigate(typeof(FileExplorer));
                    break;
                default:
                    await new MessageDialog(syncResult.ToString(), "Unhandled Error!").ShowAsync(); // TODO
                    break;
            }

            createAccount.IsEnabled = true;
            importAccountButton.IsEnabled = true;
            importProgressGroup.Visibility = Visibility.Collapsed;
            ImportProgressRing.Visibility = Visibility.Collapsed;
            ImportProgressRing.IsActive = false;
            importError.Visibility = Visibility.Visible;
        }

        private async void CreateAccount(object sender, RoutedEventArgs e) {
            createAccount.IsEnabled = false;
            importAccountButton.IsEnabled = false;
            progressGroup.Visibility = Visibility.Visible;
            progressRing.Visibility = Visibility.Visible;
            progressRing.IsActive = true;
            newAccountError.Visibility = Visibility.Collapsed;
            var createAccountResult = await App.CoreService.CreateAccount(username.Text);

            switch (createAccountResult) {
                case Core.CreateAccount.Success:
                    Frame.Navigate(typeof(FileExplorer));
                    break;
                case Core.CreateAccount.UnexpectedError ohNo:
                    await new MessageDialog(ohNo.ErrorMessage, "Unexpected Error!").ShowAsync();
                    break;
                case Core.CreateAccount.ExpectedError expectedError:
                    switch (expectedError.Error) {
                        case Core.CreateAccount.PossibleErrors.InvalidUsername:
                            newAccountError.Text = "Invalid username!";
                            newAccountError.Visibility = Visibility.Visible;
                            break;
                        case Core.CreateAccount.PossibleErrors.UsernameTaken:
                            newAccountError.Text = "Username taken!";
                            newAccountError.Visibility = Visibility.Visible;
                            break;
                        case Core.CreateAccount.PossibleErrors.CouldNotReachServer:
                            newAccountError.Text = "Could not reach server!";
                            newAccountError.Visibility = Visibility.Visible;
                            break;
                        case Core.CreateAccount.PossibleErrors.AccountExistsAlready:
                            newAccountError.Text = "An account exists already!";
                            newAccountError.Visibility = Visibility.Visible;
                            break;
                    }
                    break;
            }

            createAccount.IsEnabled = true;
            importAccountButton.IsEnabled = true;
            progressGroup.Visibility = Visibility.Collapsed;
            progressRing.Visibility = Visibility.Collapsed;
            progressRing.IsActive = false;
        }
    }
}
