using System;
using Windows.UI.Popups;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;

namespace lockbook {
    public sealed partial class SignUp : Page {
        public SignUp() {
            InitializeComponent();
        }

        public string Username {
            get {
                return usernameTextBox.Text;
            }
            set {
                usernameTextBox.Text = value;
            }
        }

        public string APILocation {
            get {
                return apiLocationTextBox.Text;
            }
            set {
                apiLocationTextBox.Text = value;
            }
        }

        public string AccountString {
            get {
                return accountStringTextBox.Text;
            }
            set {
                accountStringTextBox.Text = value;
            }
        }

        public string NewAccountError {
            get {
                return newAccountErrorTextBlock.Text;
            }
            set {
                newAccountErrorTextBlock.Visibility = string.IsNullOrEmpty(value) ? Visibility.Collapsed : Visibility.Visible;
                newAccountErrorTextBlock.Text = value;
            }
        }

        public string ImportAccountError {
            get {
                return importAccountErrorTextBlock.Text;
            }
            set {
                importAccountErrorTextBlock.Visibility = string.IsNullOrEmpty(value) ? Visibility.Collapsed : Visibility.Visible;
                importAccountErrorTextBlock.Text = value;
            }
        }

        public bool ButtonsEnabled {
            get {
                return createAccountButton.IsEnabled;
            }
            set {
                createAccountButton.IsEnabled = value;
                importAccountButton.IsEnabled = value;
            }
        }

        public bool NewAccountWorking {
            get {
                return newAccountProgressRing.IsActive;
            }
            set {
                newAccountProgressRing.IsActive = value;
                newAccountProgressRing.Visibility = value ? Visibility.Visible : Visibility.Collapsed;
                newAccountProgressGroup.Visibility = value ? Visibility.Visible : Visibility.Collapsed;
            }
        }

        public bool ImportAccountWorking {
            get {
                return importAccountProgressRing.IsActive;
            }
            set {
                importAccountProgressRing.IsActive = value;
                importAccountProgressRing.Visibility = value ? Visibility.Visible : Visibility.Collapsed;
                importAccountProgressGroup.Visibility = value ? Visibility.Visible : Visibility.Collapsed;
            }
        }

        private async void ImportAccount(object sender, RoutedEventArgs e) {
            ButtonsEnabled = false;
            ImportAccountWorking = true;

            switch (await App.CoreService.ImportAccount(AccountString)) {
                case Core.ImportAccount.Success:
                    await App.ReloadDbStateAndAccount();
                    break;
                case Core.ImportAccount.UnexpectedError error:
                    await new MessageDialog(error.ErrorMessage, "Unexpected Error!").ShowAsync();
                    break;
                case Core.ImportAccount.ExpectedError expectedError:
                    switch (expectedError.Error) {
                        case Core.ImportAccount.PossibleErrors.AccountDoesNotExist:
                            ImportAccountError = "That account does not exist on this server!";
                            break;
                        case Core.ImportAccount.PossibleErrors.AccountExistsAlready:
                            ImportAccountError = "An account exists already, clear your app data to import another account!";
                            break;
                        case Core.ImportAccount.PossibleErrors.AccountStringCorrupted:
                            ImportAccountError = "This account string is corrupt!";
                            break;
                        case Core.ImportAccount.PossibleErrors.CouldNotReachServer:
                            ImportAccountError = "Could not reach the server!";
                            break;
                        case Core.ImportAccount.PossibleErrors.UsernamePKMismatch:
                            ImportAccountError = "This username does not correspond to the public key in this account_string!";
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

            ButtonsEnabled = true;
            ImportAccountWorking = false;
        }

        private async void CreateAccount(object sender, RoutedEventArgs e) {
            ButtonsEnabled = false;
            NewAccountWorking = true;

            switch (await App.CoreService.CreateAccount(Username, APILocation)) {
                case Core.CreateAccount.Success:
                    await App.ReloadDbStateAndAccount();
                    break;
                case Core.CreateAccount.UnexpectedError error:
                    await new MessageDialog(error.ErrorMessage, "Unexpected Error!").ShowAsync();
                    break;
                case Core.CreateAccount.ExpectedError error:
                    switch (error.Error) {
                        case Core.CreateAccount.PossibleErrors.InvalidUsername:
                            NewAccountError = "Invalid username!";
                            break;
                        case Core.CreateAccount.PossibleErrors.UsernameTaken:
                            NewAccountError = "Username taken!";
                            break;
                        case Core.CreateAccount.PossibleErrors.CouldNotReachServer:
                            NewAccountError = "Could not reach server!";
                            break;
                        case Core.CreateAccount.PossibleErrors.AccountExistsAlready:
                            NewAccountError = "An account exists already!";
                            break;
                    }
                    break;
            }

            ButtonsEnabled = true;
            NewAccountWorking = false;
        }
    }
}
