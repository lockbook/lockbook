using System;
using System.Collections.Generic;
using System.Threading.Tasks;
using Windows.ApplicationModel.Activation;
using Windows.ApplicationModel.Core;
using Windows.Storage;
using Windows.UI;
using Windows.UI.Popups;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;

namespace lockbook {
    sealed partial class App : Application {
        public static CoreService CoreService;

        public App() {
            InitializeComponent();
        }

        public static Frame Frame {
            get {
                return Window.Current.Content as Frame;
            }
            set {
                Window.Current.Content = value;
            }
        }

        private static bool isOnline;
        public static bool IsOnline {
            get {
                return isOnline;
            }
            set {
                isOnline = value;
                Refresh();
            }
        }

        private static bool clientUpdateRequired;
        public static bool ClientUpdateRequired {
            get {
                return clientUpdateRequired;
            }
            set {
                clientUpdateRequired = value;
                Refresh();
            }
        }

        private static Dictionary<string, UIFile> uiFiles = new Dictionary<string, UIFile>();
        public static Dictionary<string, UIFile> UIFiles {
            get {
                return uiFiles;
            }
            set {
                uiFiles = value;
                Refresh();
            }
        }

        public static Core.Account Account { get; set; }
        public static string AccountString { get; set; }

        public static void Refresh() {
            Type targetType;
            if (ClientUpdateRequired) {
                targetType = typeof(Startup);
            } else if (Account == null) {
                targetType = typeof(SignUp);
            } else {
                targetType = typeof(FileExplorer);
            }
            if (Frame.Content.GetType() != targetType) {
                Frame.Navigate(targetType);
            }
            (Frame.Content as Startup)?.Refresh();
            (Frame.Content as FileExplorer)?.Refresh();
        }

        public static async Task SignOut() {
            await ApplicationData.Current.ClearAsync();
            await ReloadAccount();
        }

        public static async Task ReloadAccount() {
            switch (await CoreService.GetAccount()) {
                case Core.GetAccount.Success success:
                    Account = success.account;
                    break;
                case Core.GetAccount.ExpectedError expectedError:
                    switch (expectedError.Error) {
                        case Core.GetAccount.PossibleErrors.NoAccount:
                            Account = null;
                            break;
                    }
                    break;
                case Core.GetAccount.UnexpectedError error:
                    await new MessageDialog(error.ErrorMessage, "Unexpected error while loading account: " + error.ErrorMessage).ShowAsync();
                    break;
            }
            switch (await CoreService.ExportAccount()) {
                case Core.ExportAccount.Success success:
                    AccountString = success.accountString;
                    break;
                case Core.ExportAccount.ExpectedError expectedError:
                    switch (expectedError.Error) {
                        case Core.ExportAccount.PossibleErrors.NoAccount:
                            AccountString = null;
                            break;
                    }
                    break;
                case Core.ExportAccount.UnexpectedError error:
                    await new MessageDialog(error.ErrorMessage, "Unexpected error while exporting account: " + error.ErrorMessage).ShowAsync();
                    break;
            }
            Refresh();
        }

        private static async Task PromptClearState() {
            ContentDialog dialog = new ContentDialog {
                Content = "Lockbook has encountered an unexpected problem - please file a bug. Back up any unsynced files before removing your account from this device.",
                Title = "Unexpected Problem",
                IsSecondaryButtonEnabled = true,
                PrimaryButtonText = "Remove Account From This Device",
                SecondaryButtonText = "Try Using Lockbook Anyway",
            };

            var bst = new Style(typeof(Button));
            bst.Setters.Add(new Setter(Control.BackgroundProperty, Colors.Red));
            bst.Setters.Add(new Setter(Control.ForegroundProperty, Colors.White));
            dialog.PrimaryButtonStyle = bst;

            if (await dialog.ShowAsync() == ContentDialogResult.Primary) {
                await SignOut();
            }
        }

        protected override async void OnLaunched(LaunchActivatedEventArgs e)
        {
            Frame ??= new Frame();
            CoreApplication.GetCurrentView().TitleBar.ExtendViewIntoTitleBar = true;

            if (!e.PrelaunchActivated && Frame.Content == null) {
                Window.Current.Activate();
                Frame.Navigate(typeof(Startup));

                CoreService = new CoreService(ApplicationData.Current.LocalFolder.Path);

                await ReloadAccount();
            }
        }
    }
}
