using System;
using Windows.ApplicationModel.DataTransfer;
using Windows.UI.Popups;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;

namespace lockbook {

    public sealed partial class SignInContentDialog : ContentDialog {
        const ulong BYTE = 1;
        const ulong KILOBYTES = BYTE* 1000;
        const ulong MEGABYTES = KILOBYTES* 1000;
        const ulong GIGABYTES = MEGABYTES* 1000;
        const ulong TERABYTES = GIGABYTES* 1000;

        public SignInContentDialog() {
            InitializeComponent();
            setUsage();
            setUsernameAndApiUrl();
        }

        private async void setUsage() {
            var usageString = "";
            switch(await App.CoreService.GetUsage()) {
                case Core.GetUsage.Success success:
                    ulong bytes = 0;
                    foreach (var usage in success.usage) {
                        bytes += usage.byteSeconds;
                    }

                    System.Diagnostics.Debug.WriteLine(bytes + " bytes");

                    switch (bytes) {
                        case ulong inBytes when inBytes < KILOBYTES:
                            usageString = "" + inBytes + " Bytes";
                            break; 
                        case ulong inKiloBytes when inKiloBytes < MEGABYTES:
                            usageString = "" + bytes / (double) KILOBYTES + " KB";
                            break; 
                        case ulong inMegabytes when inMegabytes < GIGABYTES:
                            usageString = "" + bytes / (double) MEGABYTES + " MB";
                            break;
                        case ulong inGigaytes when inGigaytes < TERABYTES:
                            usageString = "" + bytes / (double) GIGABYTES + " GB";
                            break;
                        default:
                            usageString = "" + bytes / (double) TERABYTES + " TB";
                            break;
                    }
                    break;

                case Core.GetUsage.ExpectedError expectedError:
                    switch (expectedError.Error) {
                        case Core.GetUsage.PossibleErrors.NoAccount:
                            usageString = "No Account!";
                            break;
                        case Core.GetUsage.PossibleErrors.CouldNotReachServer:
                            usageString = "Offline!";
                            break;
                        case Core.GetUsage.PossibleErrors.ClientUpdateRequired:
                            usageString = "Update required to calculate usage!";
                            break;
                    }
                    break;

                case Core.GetUsage.UnexpectedError ohNo:
                    await new MessageDialog(ohNo.ErrorMessage, "Unexpected Error!").ShowAsync();
                    break;
            }

            spaceUsedTextBlock.Text = usageString;
        }

        public async void setUsernameAndApiUrl() {
            switch (await App.CoreService.GetAccount()) {
                case Core.GetAccount.Success success:
                    usernameTextBlock.Text = success.account.username;
                    apiTextBlock.Text = success.account.apiUrl;
                    break;

                case Core.GetAccount.ExpectedError expectedError:
                    switch (expectedError.Error) {
                        case Core.GetAccount.PossibleErrors.NoAccount:
                            usernameTextBlock.Text = "No Account!";
                            apiTextBlock.Text = "No Account!";
                            break;
                    }
                    break;

                case Core.GetAccount.UnexpectedError ohNo:
                    await new MessageDialog(ohNo.ErrorMessage, "Unexpected Error!").ShowAsync();
                    break;
            }
        }

        private async void CopyAccountStringToClipboard(object sender, RoutedEventArgs e) {
            switch (await App.CoreService.ExportAccount()) {
                case Core.ExportAccount.Success success:
                    var dataPackage = new DataPackage {
                        RequestedOperation = DataPackageOperation.Copy
                    };
                    dataPackage.SetText(success.accountString);
                    Clipboard.SetContent(dataPackage);
                    break;

                case Core.ExportAccount.ExpectedError expectedError:
                    switch (expectedError.Error) {
                        case Core.ExportAccount.PossibleErrors.NoAccount:
                            usernameTextBlock.Text = "No Account!";
                            apiTextBlock.Text = "No Account!";
                            break;
                    }
                    break;

                case Core.ExportAccount.UnexpectedError ohNo:
                    await new MessageDialog(ohNo.ErrorMessage, "Unexpected Error!").ShowAsync();
                    break;
            }
        }

        private async void ShowQRCode(object sender, RoutedEventArgs e) {
            await new MessageDialog("Whoa!", "Great work!").ShowAsync(); // TODO
        }
    }
}