using Core;
using Microsoft.UI.Xaml.Controls;
using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Collections.Specialized;
using System.ComponentModel;
using System.Linq;
using System.Threading.Tasks;
using Windows.ApplicationModel.Core;
using Windows.Storage;
using Windows.UI.Popups;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;

namespace lockbook {

    public class UIFile : INotifyPropertyChanged {
        public String Id { get; set; }
        public String Icon { get; set; }

        private bool _expanded;

        public bool Expanded {
            get {
                return _expanded;
            }

            set {
                System.Diagnostics.Debug.WriteLine("Property change1d");

                _expanded = value;
                if (PropertyChanged != null) {
                    System.Diagnostics.Debug.WriteLine("Property changed");
                    PropertyChanged(this, new PropertyChangedEventArgs("Expanded"));
                }
            }
        }

        public event PropertyChangedEventHandler PropertyChanged;

        public String Name { get; set; }

        public ObservableCollection<UIFile> Children { get; set; }
    }


    public sealed partial class FileExplorer : Page {

        public string folderGlyph = "";
        public string documentGlyph = "\uE9F9";

        public ObservableCollection<UIFile> Files = new ObservableCollection<UIFile>();

        public FileExplorer() {
            InitializeComponent();
        }


        private async void ClearStateClicked(object sender, RoutedEventArgs e) {
            await ApplicationData.Current.ClearAsync();
            CoreApplication.Exit();
        }

        private async void RefreshFiles(object sender, RoutedEventArgs e) {
            var result = await CoreService.ListFileMetadata();

            switch (result) {
                case Core.ListFileMetadata.Success success:
                    Files.Clear();
                    await PopulateTree(success.files);
                    break;
                case Core.ListFileMetadata.UnexpectedError ohNo:
                    await new MessageDialog(ohNo.errorMessage, "Unexpected Error!").ShowAsync();
                    break;
            }

        }

        private async Task PopulateTree(List<FileMetadata> coreFiles) {
            FileMetadata root = null;
            Dictionary<string, UIFile> uiFiles = new Dictionary<string, UIFile>();

            // Find our root
            foreach (var file in coreFiles) {
                if (file.Id == file.Parent) {
                    root = file;
                }
            }

            if (root == null) {
                await new MessageDialog("Root not found, file a bug report!", "Root not found!").ShowAsync();
                return;
            }

            Queue<FileMetadata> toExplore = new Queue<FileMetadata>();
            uiFiles[root.Id] = new UIFile { Id = root.Id, Name = root.Name, Icon = folderGlyph, Children = new ObservableCollection<UIFile>() };
            toExplore.Enqueue(root);
            Files.Add(uiFiles[root.Id]);

            while (toExplore.Count != 0) {
                var current = toExplore.Dequeue();

                // Find all children
                foreach (var file in coreFiles) {
                    if (current.Id == file.Parent && file.Parent != file.Id) {
                        toExplore.Enqueue(file);
                        String icon;
                        if (file.Type == "Folder") {
                            icon = folderGlyph;
                        } else {
                            icon = documentGlyph;
                        }

                        var newUi = new UIFile { Name = file.Name, Id = file.Id, Icon=icon, Children = new ObservableCollection<UIFile>() };
                        uiFiles[file.Id] = newUi;
                        uiFiles[current.Id].Children.Add(newUi);
                    }
                }
            }
        }

        private async void NewFolder(object sender, RoutedEventArgs e) {
            String tag = (String)((MenuFlyoutItem)sender).Tag;
            String name = await InputTextDialogAsync("Choose a folder name");

            var result = await CoreService.CreateFile(name, tag, FileType.Folder);
            switch (result) {
                case Core.CreateFile.Success: // TODO handle this newly created folder elegantly.
                    RefreshFiles(null, null);
                    break;
                case Core.CreateFile.ExpectedError error:
                    switch (error.error) {
                        case Core.CreateFile.PossibleErrors.FileNameNotAvailable:
                            await new MessageDialog("A file already exists at this path!", "Name Taken!").ShowAsync();
                            break;
                        case Core.CreateFile.PossibleErrors.FileNameContainsSlash:
                            await new MessageDialog("File names cannot contain slashes!", "Name Invalid!").ShowAsync();
                            break;
                        default:
                            await new MessageDialog("Unhandled Error!", error.error.ToString()).ShowAsync();
                            break;
                    }
                    break;
                case Core.CreateFile.UnexpectedError uhOh:
                    await new MessageDialog(uhOh.errorMessage, "Unexpected Error!").ShowAsync();
                    break;
            }
        }
        private async void NewDocument(object sender, RoutedEventArgs e) {
            String tag = (String)((MenuFlyoutItem)sender).Tag;
            String name = await InputTextDialogAsync("Choose a folder name");

            var result = await CoreService.CreateFile(name, tag, FileType.Document);
            switch (result) {
                case Core.CreateFile.Success: // TODO handle this newly created folder elegantly.
                    RefreshFiles(null, null);
                    break;
                case Core.CreateFile.ExpectedError error:
                    switch (error.error) {
                        case Core.CreateFile.PossibleErrors.FileNameNotAvailable:
                            await new MessageDialog("A file already exists at this path!", "Name Taken!").ShowAsync();
                            break;
                        case Core.CreateFile.PossibleErrors.FileNameContainsSlash:
                            await new MessageDialog("File names cannot contain slashes!", "Name Invalid!").ShowAsync();
                            break;
                        default:
                            await new MessageDialog("Unhandled Error!", error.error.ToString()).ShowAsync();
                            break;
                    }
                    break;
                case Core.CreateFile.UnexpectedError uhOh:
                    await new MessageDialog(uhOh.errorMessage, "Unexpected Error!").ShowAsync();
                    break;
            }
        }

        // TODO replace with nicer: https://stackoverflow.com/questions/34538637/text-input-in-message-dialog-contentdialog
        private async Task<string> InputTextDialogAsync(string title) {
            TextBox inputTextBox = new TextBox();
            inputTextBox.AcceptsReturn = false;
            inputTextBox.Height = 32;
            ContentDialog dialog = new ContentDialog();
            dialog.Content = inputTextBox;
            dialog.Title = title;
            dialog.IsSecondaryButtonEnabled = true;
            dialog.PrimaryButtonText = "Ok";
            dialog.SecondaryButtonText = "Cancel";
            if (await dialog.ShowAsync() == ContentDialogResult.Primary)
                return inputTextBox.Text;
            else
                return "";
        }

        private void FileSelected(Microsoft.UI.Xaml.Controls.TreeView sender, Microsoft.UI.Xaml.Controls.TreeViewItemInvokedEventArgs args) {
            System.Diagnostics.Debug.WriteLine("Clicked");
        }

        private void NavView_ItemInvoked(Microsoft.UI.Xaml.Controls.NavigationView sender, Microsoft.UI.Xaml.Controls.NavigationViewItemInvokedEventArgs args) {
            String tag = (String)sender.Tag;

            System.Diagnostics.Debug.WriteLine(tag);

        }

        private async void SyncCalled(object sender, Windows.UI.Xaml.Input.TappedRoutedEventArgs e) {
            sync.IsEnabled = false;
            sync.Content = "Syncing...";
            var result = await CoreService.SyncAll();
            
            switch (result) {
                case Core.SyncAll.Success:
                    RefreshFiles(null, null);
                    break;
                default:
                    await new MessageDialog(result.ToString(), "Unhandled Error!").ShowAsync(); // TODO
                    break;
            }
            sync.Content = "Sync";
            sync.IsEnabled = true;
        }
    }
}
