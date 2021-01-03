using System;
using Windows.ApplicationModel;
using Windows.ApplicationModel.Activation;
using Windows.ApplicationModel.Core;
using Windows.UI.Popups;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;
using Windows.UI.Xaml.Navigation;

namespace lockbook {
    /// <summary>
    /// Provides application-specific behavior to supplement the default Application class.
    /// </summary>
    sealed partial class App : Application {
        public static CoreService CoreService;

        /// <summary>
        /// Initializes the singleton application object.  This is the first line of authored code
        /// executed, and as such is the logical equivalent of main() or WinMain().
        /// </summary>
        public App()
        {
            InitializeComponent();
            Suspending += OnSuspending;
        }

        /// <summary>
        /// Invoked when the application is launched normally by the end user.  Other entry points
        /// will be used such as when the application is launched to open a specific file.
        /// </summary>
        /// <param name="e">Details about the launch request and process.</param>
        protected override async void OnLaunched(LaunchActivatedEventArgs e)
        {
            Frame rootFrame = Window.Current.Content as Frame;

            // Do not repeat app initialization when the Window already has content,
            // just ensure that the window is active
            if (rootFrame == null)
            {
                // Create a Frame to act as the navigation context and navigate to the first page
                rootFrame = new Frame();

                rootFrame.NavigationFailed += OnNavigationFailed;

                if (e.PreviousExecutionState == ApplicationExecutionState.Terminated)
                {
                    //TODO: Load state from previously suspended application
                }

                // Place the frame in the current Window
                Window.Current.Content = rootFrame;
            }

            CoreService = new CoreService(Windows.Storage.ApplicationData.Current.LocalFolder.Path);
            await CoreService.InitLoggerSafely();

            if (e.PrelaunchActivated == false)
            {
                if (rootFrame.Content == null)
                {
                    for(bool ready = false; !ready;) {
                        switch (await CoreService.GetDbState()) {
                            case Core.GetDbState.Success success:
                                switch (success.dbState) {
                                    case Core.DbState.ReadyToUse:
                                        rootFrame.Navigate(typeof(FileExplorer), e.Arguments);
                                        ready = true;
                                        break;
                                    case Core.DbState.Empty:
                                        rootFrame.Navigate(typeof(SignUp), e.Arguments);
                                        ready = true;
                                        break;
                                    case Core.DbState.MigrationRequired:
                                        await CoreService.MigrateDb();
                                        // todo: spinner for migration
                                        break;
                                    case Core.DbState.StateRequiresClearing:
                                        await new MessageDialog("We're embarrased about this, but your local data is corrupted and you need to reinstall Lockbook.").ShowAsync();
                                        ready = true;
                                        break;
                                }
                                break;
                            case Core.CalculateWork.UnexpectedError uhOh:
                                await new MessageDialog(uhOh.ErrorMessage, "Unexpected error during get db state: " + uhOh.ErrorMessage).ShowAsync();
                                break;
                        }
                    }
                }
                Window.Current.Activate();
                var coreTitleBar = CoreApplication.GetCurrentView().TitleBar;
                coreTitleBar.ExtendViewIntoTitleBar = true;
            }
        }

        /// <summary>
        /// Invoked when Navigation to a certain page fails
        /// </summary>
        /// <param name="sender">The Frame which failed navigation</param>
        /// <param name="e">Details about the navigation failure</param>
        void OnNavigationFailed(object sender, NavigationFailedEventArgs e)
        {
            throw new Exception("Failed to load Page " + e.SourcePageType.FullName);
        }

        /// <summary>
        /// Invoked when application execution is being suspended.  Application state is saved
        /// without knowing whether the application will be terminated or resumed with the contents
        /// of memory still intact.
        /// </summary>
        /// <param name="sender">The source of the suspend request.</param>
        /// <param name="e">Details about the suspend request.</param>
        private void OnSuspending(object sender, SuspendingEventArgs e)
        {
            var deferral = e.SuspendingOperation.GetDeferral();
            //TODO: Save application state and stop any background activity
            deferral.Complete();
        }
    }
}
