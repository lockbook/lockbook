using System;
using Windows.Foundation;
using Windows.Storage;
using Windows.UI.Popups;
using Windows.UI.ViewManagement;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;
using static lockbook.Core;

namespace lockbook {

    public sealed partial class SignUp : Page {
        public SignUp() {
            InitializeComponent();
        }

        private void ImportAccount(object sender, RoutedEventArgs e) {
            System.Diagnostics.Debug.WriteLine("Import");
        }

        private async void CreateAccount(object sender, RoutedEventArgs e) {
            createAccount.IsEnabled = false;
            progressGroup.Visibility = Visibility.Visible;
            progressRing.Visibility = Visibility.Visible;
            progressRing.IsActive = true;
            error.Visibility = Visibility.Collapsed;
            var (result, message) = await Core.CreateAccount(username.Text);

            switch (result) {
                case CreateAccountResult.Success:
                    Frame.Navigate(typeof(FileExplorer));
                    break;
                case CreateAccountResult.UnexpectedError:
                    await new MessageDialog(message, "Unexpected Error!").ShowAsync();
                    break;
                case CreateAccountResult.ContractError:
                    await new MessageDialog("See logs and file a bug report!", "Contract Error!").ShowAsync();
                    break;
                case CreateAccountResult.InvalidUsername:
                    error.Text = "Invalid username!";
                    error.Visibility = Visibility.Visible;
                    break;
                case CreateAccountResult.UsernameTaken:
                    error.Text = "Username taken!";
                    error.Visibility = Visibility.Visible;

                    break;
                case CreateAccountResult.CouldNotReachServer:
                    error.Text = "Could not reach server!";
                    error.Visibility = Visibility.Visible;
                    break;
                case CreateAccountResult.AccountExistsAlready:
                    error.Text = "An account exists already!";
                    error.Visibility = Visibility.Visible;
                    break;
            }

            createAccount.IsEnabled = true;
            progressGroup.Visibility = Visibility.Collapsed;
            progressRing.Visibility = Visibility.Collapsed;
            progressRing.IsActive = false;
        }
    }
}
