using System;
using System.Threading.Tasks;
using Windows.ApplicationModel.Activation;
using Windows.ApplicationModel.Core;
using Windows.Storage;
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

        private static Core.DbState dbState;
        public static Core.DbState DbState {
            get {
                return dbState;
            }
            set {
                dbState = value;
                Refresh();
            }
        }

        public static Core.Account Account { get; set; }
        public static string AccountString { get; set; }

        public static void Refresh() {
            (Frame.Content as Startup)?.Refresh();
            switch (DbState) {
                case Core.DbState.ReadyToUse:
                    Frame.Navigate(typeof(FileExplorer));
                    break;
                case Core.DbState.Empty:
                    Frame.Navigate(typeof(SignUp));
                    break;
                case Core.DbState.MigrationRequired:
                    Frame.Navigate(typeof(Startup));
                    break;
                case Core.DbState.StateRequiresClearing:
                    Frame.Navigate(typeof(Startup));
                    break;
            }
        }

        public static async Task SignOut() {
            await ApplicationData.Current.ClearAsync();
            await ReloadDbStateAndAccount();
        }

        public static async Task ReloadDbStateAndAccount() {
            switch (await CoreService.GetDbState()) {
                case Core.GetDbState.Success success:
                    DbState = success.dbState;
                    break;
                case Core.GetDbState.UnexpectedError error:
                    await new MessageDialog(error.ErrorMessage, "Unexpected error while getting state of local database: " + error.ErrorMessage).ShowAsync();
                    break;
            }
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
        }

        protected override async void OnLaunched(LaunchActivatedEventArgs e)
        {
            Frame ??= new Frame();
            CoreApplication.GetCurrentView().TitleBar.ExtendViewIntoTitleBar = true;

            if (!e.PrelaunchActivated && Frame.Content == null) {
                Window.Current.Activate();
                Frame.Navigate(typeof(Startup));

                CoreService = new CoreService(ApplicationData.Current.LocalFolder.Path);
                await CoreService.InitLoggerSafely();

                await ReloadDbStateAndAccount();
                if (DbState == Core.DbState.MigrationRequired) {
                    await CoreService.MigrateDb();
                    await ReloadDbStateAndAccount();
                }
            }
        }
    }
}
