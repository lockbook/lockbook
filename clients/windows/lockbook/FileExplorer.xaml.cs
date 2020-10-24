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
        public bool IsFolder() { return !IsDocument; }
        public bool Expanded { get; set; }
        public ObservableCollection<UIFile> Children { get; set; }
    }

    public sealed partial class FileExplorer : Page {

        public string currentDocumentId = "";

        public string folderGlyph = "\uED25";
        public string documentGlyph = "\uE9F9";
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
                        int work = success.workCalculated.workUnits.Count;
                        if (syncContainer.IsEnabled) {
                            if (work == 0) {
                                syncIcon.Glyph = checkGlyph;
                                syncText.Text = "Up to date!";
                            } else {
                                syncIcon.Glyph = syncGlyph;
                                if (work == 1)
                                    syncText.Text = work + " item need to be synced.";
                                else
                                    syncText.Text = work + " items need to be synced.";
                            }
                        }
                        break;
                    case Core.CalculateWork.ExpectedError error:
                        switch (error.Error) {
                            case Core.CalculateWork.PossibleErrors.CouldNotReachServer:
                                if (syncContainer.IsEnabled) {
                                    syncIcon.Glyph = offlineGlyph;
                                    syncText.Text = "Offline";
                                }
                                break;
                            default:
                                System.Diagnostics.Debug.WriteLine("Unexpected error during calc work loop: " + error.Error);
                                break;

                        }
                        break;
                    case Core.CalculateWork.UnexpectedError uhOh:
                        System.Diagnostics.Debug.WriteLine("Unexpected error during calc work loop: " + uhOh.ErrorMessage);
                        break;
                }

                await Task.Delay(2000);
            }
        }

        private async Task RefreshFiles() {
            var result = await App.CoreService.ListMetadatas();

            switch (result) {
                case Core.ListMetadatas.Success success:
                    await PopulateTree(success.files);
                    break;
                case Core.ListMetadatas.UnexpectedError ohNo:
                    await new MessageDialog(ohNo.ErrorMessage, "Unexpected Error!").ShowAsync();
                    break;
            }

        }

        // TODO consider diffing trees and performing the min update to prevent the UI from flashing
        private async Task PopulateTree(List<FileMetadata> coreFiles) {

            var expandedItems = new HashSet<string>();

            foreach (var file in uiFiles.Values) {
                if (file.Expanded)
                    expandedItems.Add(file.Id);
            }

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
            uiFiles[root.Id] = new UIFile { Icon = rootGlyph, Expanded = true, Id = root.Id, Name = root.Name, IsDocument = false, Children = new ObservableCollection<UIFile>() };
            toExplore.Enqueue(root);

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

                        var expanded = expandedItems.Contains(file.Id);

                        var newUi = new UIFile { Name = file.Name, Id = file.Id, Icon = icon, IsDocument = file.Type == "Document", Children = children, Expanded = expanded };
                        uiFiles[file.Id] = newUi;
                        uiFiles[current.Id].Children.Add(newUi);
                    }
                }
            }

            Files.Clear();
            Files.Add(uiFiles[root.Id]);
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
                    switch (error.Error) {
                        case Core.CreateFile.PossibleErrors.FileNameNotAvailable:
                            await new MessageDialog("A file already exists at this path!", "Name Taken!").ShowAsync();
                            break;
                        case Core.CreateFile.PossibleErrors.FileNameContainsSlash:
                            await new MessageDialog("File names cannot contain slashes!", "Name Invalid!").ShowAsync();
                            break;
                        case Core.CreateFile.PossibleErrors.FileNameEmpty:
                            await new MessageDialog("File names cannot be empty!", "Name Empty!").ShowAsync();
                            break;
                        default:
                            await new MessageDialog("Unhandled Error!", error.Error.ToString()).ShowAsync();
                            break;
                    }
                    break;
                case Core.CreateFile.UnexpectedError uhOh:
                    await new MessageDialog(uhOh.ErrorMessage, "Unexpected Error!").ShowAsync();
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
            syncContainer.IsEnabled = false;
            syncText.Text = "Syncing...";

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
            syncText.Text = "Up to date!";
            syncContainer.IsEnabled = true;
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
                    switch (error.Error) {
                        case Core.RenameFile.PossibleErrors.FileNameNotAvailable:
                            await new MessageDialog("A file already exists at this path!", "Name Taken!").ShowAsync();
                            break;
                        case Core.RenameFile.PossibleErrors.NewNameContainsSlash:
                            await new MessageDialog("File names cannot contain slashes!", "Invalid Name!").ShowAsync();
                            break;
                        case Core.RenameFile.PossibleErrors.FileDoesNotExist:
                            await new MessageDialog("Could not locate the file you're trying to rename! Please file a bug report.", "Unexpected Error!").ShowAsync();
                            break;
                        case Core.RenameFile.PossibleErrors.NewNameEmpty:
                            await new MessageDialog("New name cannot be empty!", "File name empty!").ShowAsync();
                            break;
                    }
                    break;
                case Core.RenameFile.UnexpectedError uhOh:
                    await new MessageDialog(uhOh.ErrorMessage, "Unexpected Error!").ShowAsync();
                    break;
            }
        }

        private async void Unimplemented(object sender, RoutedEventArgs e) {
            await new MessageDialog("Parth has not implemented this yet!", "Sorry!").ShowAsync();
        }

        // Move things
        // TODO supporting multiple file moves would simply require iterating through this list, 
        // perhaps also it would require figuring out if a move is going to fail, maybe staging it
        // as a transaction or something like that could be what we need to do this.
        private void NavigationViewItem_DragStarting(UIElement sender, Microsoft.UI.Xaml.Controls.TreeViewDragItemsStartingEventArgs args) {
            string id = (args.Items[0] as UIFile)?.Id;
            System.Diagnostics.Debug.WriteLine("drag starting: " + args.Items[0]);

            if (id != null) {
                args.Data.SetData("id", id); // TODO we do not need to do this
            } else {
                System.Diagnostics.Debug.WriteLine("tag was null");
            }
        }

        private void NavigationViewItem_DragOver(object sender, DragEventArgs e) {
            e.AcceptedOperation = DataPackageOperation.Move; // TODO show none over documents
        }

        private async void NavigationViewItem_Drop(object sender, Microsoft.UI.Xaml.Controls.TreeViewDragItemsCompletedEventArgs e) {
            string toMove = (e.Items[0] as UIFile)?.Id;
            string newParent = (e.NewParentItem as UIFile)?.Id;

            var result = await App.CoreService.MoveFile(toMove, newParent);

            switch (result) {
                case Core.MoveFile.Success:
                    break;
                case Core.MoveFile.ExpectedError error:
                    switch (error.Error) {
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
                        case Core.MoveFile.PossibleErrors.CannotMoveRoot:
                            await new MessageDialog("Cannot move root folder!", "Cannot move root!").ShowAsync();
                            break;
                    }
                    break;
                case Core.MoveFile.UnexpectedError uhOh:
                    await new MessageDialog(uhOh.ErrorMessage, "Unexpected Error!").ShowAsync();
                    break;
            }

            await RefreshFiles();
        }

        private async void DocumentSelected(object sender, Windows.UI.Xaml.Input.TappedRoutedEventArgs e) {
            string tag = (string)((FrameworkElement)sender).Tag;
            var file = uiFiles[tag];

            if (file.IsDocument) {
                currentDocumentId = tag;
                var result = await App.CoreService.ReadDocument(currentDocumentId);

                switch (result) {
                    case Core.ReadDocument.Success content:
                        editor.TextDocument.SetText(TextSetOptions.None, content.content.secret);
                        editor.TextDocument.ClearUndoRedoHistory();
                        keyStrokeCount[tag] = 0;
                        break;
                    case Core.ReadDocument.ExpectedError error:
                        switch (error.Error) {
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
                        await new MessageDialog(uhOh.ErrorMessage, "Unexpected Error!").ShowAsync();
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
                        switch (error.Error) {
                            case Core.WriteDocument.PossibleErrors.NoAccount:
                                await new MessageDialog("No account found! Please file a bug report.", "Unexpected Error!").ShowAsync();
                                break;
                            case Core.WriteDocument.PossibleErrors.FolderTreatedAsDocument:
                                await new MessageDialog("You cannot read a folder, please file a bug report!", "Bad read target!").ShowAsync();
                                break;
                            case Core.WriteDocument.PossibleErrors.FileDoesNotExist:
                                await new MessageDialog("Could not locate the file you're trying to edit! Please file a bug report.", "Unexpected Error!").ShowAsync();
                                break;
                        }
                        break;
                    case Core.WriteDocument.UnexpectedError uhOh:
                        await new MessageDialog(uhOh.ErrorMessage, "Unexpected Error!").ShowAsync();
                        break;
                }
            }
        }


        private void TreeView_DragItemsStarting_1(Microsoft.UI.Xaml.Controls.TreeView sender, Microsoft.UI.Xaml.Controls.TreeViewDragItemsStartingEventArgs args) {

        }

        private void TreeView_DragItemsStarting(Microsoft.UI.Xaml.Controls.TreeView sender, Microsoft.UI.Xaml.Controls.TreeViewDragItemsStartingEventArgs args) {

        }

        private void TreeView_DragItemsCompleted(Microsoft.UI.Xaml.Controls.TreeView sender, Microsoft.UI.Xaml.Controls.TreeViewDragItemsCompletedEventArgs args) {

        }
    }
}
