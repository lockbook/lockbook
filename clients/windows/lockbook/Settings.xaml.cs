using QRCoder;
using System;
using System.Threading.Tasks;
using Windows.ApplicationModel.DataTransfer;
using Windows.Storage.Streams;
using Windows.UI.Popups;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;
using Windows.UI.Xaml.Media.Imaging;

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
            switch (await App.CoreService.ExportAccount()) {
                case Core.ExportAccount.Success success:
                    showQRCode.IsEnabled = false;
                    // https://github.com/codebude/QRCoder/wiki/Advanced-usage---QR-Code-renderers#24-bitmapbyteqrcode-renderer-in-detail
                    var qrGenerator = new QRCodeGenerator();
                    var qrCodeData = qrGenerator.CreateQrCode(success.accountString, QRCodeGenerator.ECCLevel.Q);
                    var qrCode = new BitmapByteQRCode(qrCodeData);
                    var qrCodeAsBitmapByteArr = await Task.Run(() => qrCode.GetGraphic(20));

                    using (InMemoryRandomAccessStream stream = new InMemoryRandomAccessStream()) {
                        using (DataWriter writer = new DataWriter(stream.GetOutputStreamAt(0))) {
                            writer.WriteBytes(qrCodeAsBitmapByteArr);
                            await writer.StoreAsync();
                        }
                        var image = new BitmapImage();
                        await image.SetSourceAsync(stream);

                        qrCodeImg.Source = image;
                    }
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
    }
}