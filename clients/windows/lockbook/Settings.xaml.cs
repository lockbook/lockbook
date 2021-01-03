using QRCoder;
using System;
using System.Threading.Tasks;
using Windows.ApplicationModel.DataTransfer;
using Windows.Storage.Streams;
using Windows.UI.Popups;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;
using Windows.UI.Xaml.Media;
using Windows.UI.Xaml.Media.Imaging;

namespace lockbook {

    public sealed partial class SignInContentDialog : ContentDialog {
        const ulong BYTE = 1;
        const ulong KILOBYTES = BYTE * 1000;
        const ulong MEGABYTES = KILOBYTES * 1000;
        const ulong GIGABYTES = MEGABYTES * 1000;
        const ulong TERABYTES = GIGABYTES * 1000;

        public string Username {
            get {
                return usernameTextBlock.Text;
            }
            set {
                usernameTextBlock.Text = value;
            }
        }

        public string ServerLocation {
            get {
                return serverLocationTextBlock.Text;
            }
            set {
                serverLocationTextBlock.Text = value;
            }
        }

        public string SpaceUsed {
            get {
                return spaceUsedTextBlock.Text;
            }
            set {
                spaceUsedTextBlock.Text = value;
            }
        }

        public ImageSource AccountQRCode {
            get {
                return qrCodeImg.Source;
            }
            set {
                qrCodeImg.Source = value;
            }
        }

        public SignInContentDialog() {
            InitializeComponent();
            setUsage();
            setUsernameAndApiUrl();
        }

        public static string SpaceUsedStringFromUsageBytes(ulong usageBytes) {
            switch (usageBytes) {
                case ulong inBytes when inBytes < KILOBYTES:
                    return "" + usageBytes + " Bytes";
                case ulong inKiloBytes when inKiloBytes < MEGABYTES:
                    return "" + usageBytes / (double)KILOBYTES + " KB";
                case ulong inMegabytes when inMegabytes < GIGABYTES:
                    return "" + usageBytes / (double)MEGABYTES + " MB";
                case ulong inGigaytes when inGigaytes < TERABYTES:
                    return "" + usageBytes / (double)GIGABYTES + " GB";
                default:
                    return "" + usageBytes / (double)TERABYTES + " TB";
            }
        }

        private async void setUsage() {
            var usageString = "";
            switch (await App.CoreService.GetUsage()) {
                case Core.GetUsage.Success success:
                    ulong bytes = 0;
                    foreach (var usage in success.usage) {
                        bytes += usage.byteSeconds;
                    }

                    System.Diagnostics.Debug.WriteLine(bytes + " bytes");

                    usageString = SpaceUsedStringFromUsageBytes(bytes);
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

            SpaceUsed = usageString;
        }

        public async void setUsernameAndApiUrl() {
            switch (await App.CoreService.GetAccount()) {
                case Core.GetAccount.Success success:
                    Username = success.account.username;
                    ServerLocation = success.account.apiUrl;
                    break;

                case Core.GetAccount.ExpectedError expectedError:
                    switch (expectedError.Error) {
                        case Core.GetAccount.PossibleErrors.NoAccount:
                            usernameTextBlock.Text = "No Account!";
                            serverLocationTextBlock.Text = "No Account!";
                            break;
                    }
                    break;

                case Core.GetAccount.UnexpectedError ohNo:
                    await new MessageDialog(ohNo.ErrorMessage, "Unexpected Error!").ShowAsync();
                    break;
            }
        }

        private static void CopyToClipboard(string s) {
            var dataPackage = new DataPackage {
                RequestedOperation = DataPackageOperation.Copy
            };
            dataPackage.SetText(s);
            Clipboard.SetContent(dataPackage);
        }

        private async void CopyAccountStringToClipboard(object sender, RoutedEventArgs e) {
            switch (await App.CoreService.ExportAccount()) {
                case Core.ExportAccount.Success success:
                    CopyToClipboard(success.accountString);
                    break;

                case Core.ExportAccount.ExpectedError expectedError:
                    switch (expectedError.Error) {
                        case Core.ExportAccount.PossibleErrors.NoAccount:
                            usernameTextBlock.Text = "No Account!";
                            serverLocationTextBlock.Text = "No Account!";
                            break;
                    }
                    break;

                case Core.ExportAccount.UnexpectedError ohNo:
                    await new MessageDialog(ohNo.ErrorMessage, "Unexpected Error!").ShowAsync();
                    break;
            }
        }

        private static async Task<ImageSource> GenerateQRCode(string accountString) {
            // https://github.com/codebude/QRCoder/wiki/Advanced-usage---QR-Code-renderers#24-bitmapbyteqrcode-renderer-in-detail
            var qrGenerator = new QRCodeGenerator();
            var qrCodeData = qrGenerator.CreateQrCode(accountString, QRCodeGenerator.ECCLevel.Q);
            var qrCode = new BitmapByteQRCode(qrCodeData);
            var qrCodeAsBitmapByteArr = await Task.Run(() => qrCode.GetGraphic(20));

            using (InMemoryRandomAccessStream stream = new InMemoryRandomAccessStream()) {
                using (DataWriter writer = new DataWriter(stream.GetOutputStreamAt(0))) {
                    writer.WriteBytes(qrCodeAsBitmapByteArr);
                    await writer.StoreAsync();
                }
                var image = new BitmapImage();
                await image.SetSourceAsync(stream);

                return image;
            }
        }

        private async void ShowQRCode(object sender, RoutedEventArgs e) {
            switch (await App.CoreService.ExportAccount()) {
                case Core.ExportAccount.Success success:
                    showQRCode.IsEnabled = false;
                    qrCodeImg.Source = await GenerateQRCode(success.accountString);
                    break;

                case Core.ExportAccount.ExpectedError expectedError:
                    switch (expectedError.Error) {
                        case Core.ExportAccount.PossibleErrors.NoAccount:
                            usernameTextBlock.Text = "No Account!";
                            serverLocationTextBlock.Text = "No Account!";
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