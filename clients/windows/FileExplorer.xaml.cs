using Core;
using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Threading.Tasks;
using Windows.ApplicationModel.Core;
using Windows.ApplicationModel.DataTransfer;
using Windows.Storage;
using Windows.UI.Popups;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;

namespace lockbook {

    public class UIFile {
        public String Id { get; set; }

        public String Icon { get; set; }

        public String Name { get; set; }

        public ObservableCollection<UIFile> Children { get; set; }
    }


    public sealed partial class FileExplorer : Page {

        public string folderGlyph = ""; // "\uED25";
        public string documentGlyph = ""; //"\uE9F9";
        public string rootGlyph = "\uEC25";

        ObservableCollection<UIFile> Files = new ObservableCollection<UIFile>();
        Dictionary<string, UIFile> uiFiles = new Dictionary<string, UIFile>();

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
                    await PopulateTree(success.files);
                    break;
                case Core.ListFileMetadata.UnexpectedError ohNo:
                    await new MessageDialog(ohNo.errorMessage, "Unexpected Error!").ShowAsync();
                    break;
            }

        }

        private async Task PopulateTree(List<FileMetadata> coreFiles) {
            Files.Clear();
            uiFiles.Clear();

            FileMetadata root = null;

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
            uiFiles[root.Id] = new UIFile { Id = root.Id, Name = root.Name, Icon = rootGlyph, Children = new ObservableCollection<UIFile>() };
            toExplore.Enqueue(root);
            Files.Add(uiFiles[root.Id]);

            while (toExplore.Count != 0) {
                var current = toExplore.Dequeue();

                // Find all children
                foreach (var file in coreFiles) {
                    if (current.Id == file.Parent && file.Parent != file.Id) {
                        toExplore.Enqueue(file);

                        String icon;
                        ObservableCollection<UIFile> children;

                        if (file.Type == "Folder") {
                            icon = folderGlyph;
                            children = new ObservableCollection<UIFile>();
                        } else {
                            icon = documentGlyph;
                            children = null;
                        }

                        var newUi = new UIFile { Name = file.Name, Id = file.Id, Icon = icon, Children = children };
                        uiFiles[file.Id] = newUi;
                        uiFiles[current.Id].Children.Add(newUi);
                    }
                }
            }
        }

        private async void NewFolder(object sender, RoutedEventArgs e) {
            String parent = (String)((MenuFlyoutItem)sender).Tag;
            String name = await InputTextDialogAsync("Choose a folder name");

            await AddFile(FileType.Folder, name, parent);
        }
        private async void NewDocument(object sender, RoutedEventArgs e) {
            String parent = (String)((MenuFlyoutItem)sender).Tag;
            String name = await InputTextDialogAsync("Choose a document name");

            await AddFile(FileType.Document, name, parent);
        }

        private async Task AddFile(FileType type, String name, String parent) {
            var result = await CoreService.CreateFile(name, parent, type);
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
            TextBox inputTextBox = new TextBox {
                AcceptsReturn = false,
                Height = 32
            };
            ContentDialog dialog = new ContentDialog {
                Content = inputTextBox,
                Title = title,
                IsSecondaryButtonEnabled = true,
                PrimaryButtonText = "Ok",
                SecondaryButtonText = "Cancel",
            };
            if (await dialog.ShowAsync() == ContentDialogResult.Primary)
                return inputTextBox.Text;
            else
                return "";
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



        private async void RenameFile(object sender, RoutedEventArgs e) {
            String id = (String)((MenuFlyoutItem)sender).Tag;
            String newName = await InputTextDialogAsync("Choose a new name");

            var result = await CoreService.RenameFile(id, newName);

            switch (result) {
                case Core.RenameFile.Success:
                    RefreshFiles(null, null);
                    break;
                case Core.RenameFile.ExpectedError error:
                    switch (error.error) {
                        case Core.RenameFile.PossibleErrors.FileNameNotAvailable:
                            await new MessageDialog("A file already exists at this path!", "Name Taken!").ShowAsync();
                            break;
                        case Core.RenameFile.PossibleErrors.NewNameContainsSlash:
                            await new MessageDialog("File names cannot contain slashes!", "Invalid Name!").ShowAsync();
                            break;
                        case Core.RenameFile.PossibleErrors.FileDoesNotExist:
                            await new MessageDialog("Could not locate the file you're trying to rename! Please file a bug report.f", "Unexpected Error!").ShowAsync();
                            break;
                    }
                    break;
                case Core.RenameFile.UnexpectedError uhOh:
                    await new MessageDialog(uhOh.errorMessage, "Unexpected Error!").ShowAsync();
                    break;
            }
        }

        private async void Unimplemented(object sender, RoutedEventArgs e) {
            await new MessageDialog("Parth has not implemented this yet!", "Sorry!").ShowAsync();
        }

        // Move things
        private void NavigationViewItem_DragStarting(UIElement sender, DragStartingEventArgs args) {
            string tag = (string)((sender as FrameworkElement)?.Tag);

            if (tag != null) {
                args.AllowedOperations = DataPackageOperation.Move;
                args.Data.SetData("id", tag);
            }

        }
        // how to do this good: https://stackoverflow.com/a/48176944/1060955
        private void NavigationViewItem_DragOver(object sender, DragEventArgs e) {
            e.AcceptedOperation = DataPackageOperation.Move;
        }

        private async void NavigationViewItem_Drop(object sender, DragEventArgs e) {
            if ((e.OriginalSource as FrameworkElement)?.Tag is String newParent) {
                if (await (e.DataView.GetDataAsync("id")) is String oldFileId) {
                    System.Diagnostics.Debug.WriteLine("Source: " + oldFileId);
                    System.Diagnostics.Debug.WriteLine("target: " + newParent);
                    e.Handled = true;
                }
            }
        }

        private void FileSelected(object sender, Windows.UI.Xaml.Input.TappedRoutedEventArgs e) {
            String tag = (String) (sender as FrameworkElement).Tag;

            System.Diagnostics.Debug.WriteLine(tag);
        }
    }
}
