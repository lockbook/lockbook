using Core;
using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Threading.Tasks;
using Windows.ApplicationModel.Core;
using Windows.ApplicationModel.DataTransfer;
using Windows.Storage;
using Windows.UI.Popups;
using Windows.UI.Text;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;

namespace lockbook {

    public class UIFile {
        public string Id { get; set; }

        public string Icon { get; set; }

        public string Name { get; set; }

        public bool IsDocument { get; set; }

        public ObservableCollection<UIFile> Children { get; set; }
    }


    public sealed partial class FileExplorer : Page {

        public string currentDocumentId = "";

        public string folderGlyph = ""; // "\uED25";
        public string documentGlyph = ""; //"\uE9F9";
        public string rootGlyph = "\uEC25";
        public string checkGlyph = "\uE73E";
        public string syncGlyph = "\uE895";
        public string offlineGlyph = "\uF384";

        ObservableCollection<UIFile> Files = new ObservableCollection<UIFile>();
        Dictionary<string, UIFile> uiFiles = new Dictionary<string, UIFile>();
        Dictionary<string, int> keyStrokeCount = new Dictionary<string, int>();

        public FileExplorer() {
            InitializeComponent();
        }

        private async void ClearStateClicked(object sender, RoutedEventArgs e) {
            await ApplicationData.Current.ClearAsync();
            CoreApplication.Exit();
        }

        private async void NavigationViewLoaded(object sender, RoutedEventArgs e) {
            await RefreshFiles();
            CheckForWorkLoop();
        }

        private async void CheckForWorkLoop() {
            while (true) {
                var result = await App.CoreService.CalculateWork();

                switch (result) {
                    case Core.CalculateWork.Success success:
                        int work = success.workCalculated.WorkUnits.Count;
                        if (sync.IsEnabled) {
                            if (work == 0) {
                                syncIcon.Glyph = checkGlyph;
                                sync.Content = "Up to date!";
                            } else {
                                syncIcon.Glyph = syncGlyph;
                                if (work == 1)
                                    sync.Content = work + " item need to be synced.";
                                else
                                    sync.Content = work + " items need to be synced.";
                            }
                        }
                        break;
                    case Core.CalculateWork.ExpectedError error:
                        switch (error.error) {
                            case Core.CalculateWork.PossibleErrors.CouldNotReachServer:
                                if (sync.IsEnabled) {
                                    syncIcon.Glyph = offlineGlyph;
                                    sync.Content = "Offline";
                                }
                                break;
                            default:
                                System.Diagnostics.Debug.WriteLine("Unexpected error during calc work loop: " + error.error);
                                break;

                        }
                        break;
                    case Core.CalculateWork.UnexpectedError uhOh:
                        System.Diagnostics.Debug.WriteLine("Unexpected error during calc work loop: " + uhOh.errorMessage);
                        break;
                }

                await Task.Delay(2000);
            }
        }

        private async Task RefreshFiles() {
            var result = await App.CoreService.ListFileMetadata();

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

            // Explore and find children
            Queue<FileMetadata> toExplore = new Queue<FileMetadata>();
            uiFiles[root.Id] = new UIFile { Id = root.Id, Name = root.Name, IsDocument = false, Icon = rootGlyph, Children = new ObservableCollection<UIFile>() };
            toExplore.Enqueue(root);
            Files.Add(uiFiles[root.Id]);

            while (toExplore.Count != 0) {
                var current = toExplore.Dequeue();

                // Find all children
                foreach (var file in coreFiles) {
                    if (current.Id == file.Parent && file.Parent != file.Id) {
                        toExplore.Enqueue(file);

                        string icon;
                        ObservableCollection<UIFile> children;

                        if (file.Type == "Folder") {
                            icon = folderGlyph;
                            children = new ObservableCollection<UIFile>();
                        } else {
                            icon = documentGlyph;
                            children = null;
                        }

                        var newUi = new UIFile { Name = file.Name, Id = file.Id, Icon = icon, IsDocument = file.Type == "Document", Children = children };
                        uiFiles[file.Id] = newUi;
                        uiFiles[current.Id].Children.Add(newUi);
                    }
                }
            }
        }

        private async void NewFolder(object sender, RoutedEventArgs e) {
            string parent = (string)((MenuFlyoutItem)sender).Tag;
            string name = await InputTextDialogAsync("Choose a folder name");

            await AddFile(FileType.Folder, name, parent);
        }
      
        private async void NewDocument(object sender, RoutedEventArgs e) {
            string parent = (string)((MenuFlyoutItem)sender).Tag;
            string name = await InputTextDialogAsync("Choose a document name");

            await AddFile(FileType.Document, name, parent);
        }

        private async Task AddFile(FileType type, string name, string parent) {
            var result = await App.CoreService.CreateFile(name, parent, type);
            switch (result) {
                case Core.CreateFile.Success: // TODO handle this newly created folder elegantly.
                    await RefreshFiles();
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

            var result = await App.CoreService.SyncAll();

            switch (result) {
                case Core.SyncAll.Success:
                    await RefreshFiles();
                    break;
                default:
                    await new MessageDialog(result.ToString(), "Unhandled Error!").ShowAsync(); // TODO
                    break;
            }
            syncIcon.Glyph = checkGlyph;
            sync.Content = "Upto date!";
            sync.IsEnabled = true;
        }

        private async void RenameFile(object sender, RoutedEventArgs e) {
            string id = (string)((MenuFlyoutItem)sender).Tag;
            string newName = await InputTextDialogAsync("Choose a new name");

            var result = await App.CoreService.RenameFile(id, newName);

            switch (result) {
                case Core.RenameFile.Success:
                    await RefreshFiles();
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
                            await new MessageDialog("Could not locate the file you're trying to rename! Please file a bug report.", "Unexpected Error!").ShowAsync();
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
        
        private void NavigationViewItem_DragOver(object sender, DragEventArgs e) {
            e.AcceptedOperation = DataPackageOperation.Move;
        }

        private async void NavigationViewItem_Drop(object sender, DragEventArgs e) {
            if ((e.OriginalSource as FrameworkElement)?.Tag is string newParent) {
                if (await (e.DataView.GetDataAsync("id")) is string oldFileId) {
                    e.Handled = true;

                    var result = await App.CoreService.MoveFile(oldFileId, newParent);

                    switch (result) {
                        case Core.MoveFile.Success:
                            await RefreshFiles();
                            break;
                        case Core.MoveFile.ExpectedError error:
                            switch (error.error) {
                                case Core.MoveFile.PossibleErrors.NoAccount:
                                    await new MessageDialog("No account found! Please file a bug report.", "Unexpected Error!").ShowAsync();
                                    break;
                                case Core.MoveFile.PossibleErrors.DocumentTreatedAsFolder:
                                    await new MessageDialog("You cannot move a file into a document", "Bad move destination!").ShowAsync();
                                    break;
                                case Core.MoveFile.PossibleErrors.FileDoesNotExist:
                                    await new MessageDialog("Could not locate the file you're trying to move! Please file a bug report.", "Unexpected Error!").ShowAsync();
                                    break;
                                case Core.MoveFile.PossibleErrors.TargetParentHasChildNamedThat:
                                    await new MessageDialog("A file with that name exists at the target location!", "Name conflict!").ShowAsync();
                                    break;
                                case Core.MoveFile.PossibleErrors.TargetParentDoesNotExist:
                                    await new MessageDialog("Could not locate the file you're trying to move to! Please file a bug report.", "Unexpected Error!").ShowAsync();
                                    break;
                            }
                            break;
                        case Core.MoveFile.UnexpectedError uhOh:
                            await new MessageDialog(uhOh.errorMessage, "Unexpected Error!").ShowAsync();
                            break;
                    }
                }
            }
        }

        private async void DocumentSelected(Microsoft.UI.Xaml.Controls.NavigationView sender, Microsoft.UI.Xaml.Controls.NavigationViewSelectionChangedEventArgs args) {
            string tag = (string)args.SelectedItemContainer?.Tag;

            if (tag != null) {
                currentDocumentId = (string)args.SelectedItemContainer.Tag;
                var result = await App.CoreService.ReadDocument(currentDocumentId);

                switch (result) {
                    case Core.ReadDocument.Success content:
                        editor.TextDocument.SetText(TextSetOptions.None, content.content.secret);
                        editor.TextDocument.ClearUndoRedoHistory();
                        keyStrokeCount[tag] = 0;
                        break;
                    case Core.ReadDocument.ExpectedError error:
                        switch (error.error) {
                            case Core.ReadDocument.PossibleErrors.NoAccount:
                                await new MessageDialog("No account found! Please file a bug report.", "Unexpected Error!").ShowAsync();
                                break;
                            case Core.ReadDocument.PossibleErrors.TreatedFolderAsDocument:
                                await new MessageDialog("You cannot read a folder, please file a bug report!", "Bad read target!").ShowAsync();
                                break;
                            case Core.ReadDocument.PossibleErrors.FileDoesNotExist:
                                await new MessageDialog("Could not locate the file you're trying to edit! Please file a bug report.", "Unexpected Error!").ShowAsync();
                                break;
                        }
                        break;
                    case Core.ReadDocument.UnexpectedError uhOh:
                        await new MessageDialog(uhOh.errorMessage, "Unexpected Error!").ShowAsync();
                        break;
                }
            }
        }

        private async void TextChanged(object sender, RoutedEventArgs e) {
            if (currentDocumentId != "") {
                string docID = currentDocumentId;
                string text;
                editor.TextDocument.GetText(TextGetOptions.UseLf, out text);

                // Only save the document if no keystrokes have happened in the last 1 second
                keyStrokeCount[docID]++;
                var current = keyStrokeCount[docID];
                await Task.Delay(750);
                if (current != keyStrokeCount[docID]) {
                    return;
                }

                var result = await App.CoreService.WriteDocument(docID, text);

                switch (result) {
                    case Core.WriteDocument.Success:
                        break;
                    case Core.WriteDocument.ExpectedError error:
                        switch (error.error) {
                            case Core.WriteDocument.PossibleErrors.NoAccount:
                                await new MessageDialog("No account found! Please file a bug report.", "Unexpected Error!").ShowAsync();
                                break;
                            case Core.WriteDocument.PossibleErrors.TreatedFolderAsDocument:
                                await new MessageDialog("You cannot read a folder, please file a bug report!", "Bad read target!").ShowAsync();
                                break;
                            case Core.WriteDocument.PossibleErrors.FileDoesNotExist:
                                await new MessageDialog("Could not locate the file you're trying to edit! Please file a bug report.", "Unexpected Error!").ShowAsync();
                                break;
                        }
                        break;
                    case Core.WriteDocument.UnexpectedError uhOh:
                        await new MessageDialog(uhOh.errorMessage, "Unexpected Error!").ShowAsync();
                        break;
                }
            }
        }
    }
}
