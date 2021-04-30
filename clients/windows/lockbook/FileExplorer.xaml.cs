using Core;
using lib;
using Newtonsoft.Json;
using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Linq;
using System.Threading.Tasks;
using Windows.ApplicationModel.DataTransfer;
using Windows.Foundation;
using Windows.UI;
using Windows.UI.Core;
using Windows.UI.Input.Inking;
using Windows.UI.Popups;
using Windows.UI.Text;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;

namespace lockbook {
    public class UIFile {
        public const string folderGlyph = "\uED25";
        public const string documentGlyph = "\uE9F9";
        public const string rootGlyph = "\uEC25";

        public bool IsRoot { get; set; }
        public string Id { get; set; }
        public string Icon {
            get {
                return IsRoot ? rootGlyph : (IsDocument ? documentGlyph : folderGlyph);
            }
            set { } // required for Xaml
        }
        public string Name { get; set; }
        public bool IsDocument { get; set; }
        public bool IsFolder {
            get {
                return !IsDocument;
            }
            set {
                IsDocument = !value;
            }
        }
        public bool IsExpanded { get; set; }
        public ObservableCollection<UIFile> Children { get; set; }
    }

    public sealed partial class FileExplorer : Page {
        Symbol TouchWriting = (Symbol)0xED5F;

        public string SelectedDocumentId { get; set; } = "";
        private int itemsToSync;
        public int ItemsToSync {
            get {
                return itemsToSync;
            }
            set {
                itemsToSync = value;
                Refresh();
            }
        }
        public bool SyncWorking {
            get {
                return !syncContainer.IsEnabled;
            }
            set {
                syncContainer.IsEnabled = !value;
                Refresh();
            }
        }
        public enum EditMode {
            None,
            Text,
            Draw
        }
        private EditMode currentEditMode;
        public EditMode CurrentEditMode {
            get {
                return currentEditMode;
            }
            set {
                currentEditMode = value;
                Refresh();
            }
        }
        private Dictionary<ColorAlias, ColorRGB> theme;
        public ColorAlias DrawingColor {
            get {
                return theme.GetColorAlias(inkCanvas.InkPresenter.CopyDefaultDrawingAttributes().Color);
            }
            set {
                inkCanvas.InkPresenter.UpdateDefaultDrawingAttributes(new InkDrawingAttributes {
                    Color = theme.GetUIColor(value, 0xFF),
                    Size = new Size(20, 20),
                });
            }
        }

        public const string checkGlyph = "\uE73E";
        public const string syncGlyph = "\uE895";
        public const string offlineGlyph = "\uF384";

        ObservableCollection<UIFile> Files = new ObservableCollection<UIFile>();
        Dictionary<string, int> editCount = new Dictionary<string, int>();
        DrawingContext drawingContext;

        public FileExplorer() {
            InitializeComponent();
            Files.Add(App.UIFiles.FirstOrDefault(kvp => kvp.Value.IsRoot).Value);
            inkCanvas.InkPresenter.InputDeviceTypes =
                CoreInputDeviceTypes.Mouse |
                CoreInputDeviceTypes.Pen |
                CoreInputDeviceTypes.Touch;
            inkCanvas.InkPresenter.UpdateDefaultDrawingAttributes(new InkDrawingAttributes {
                Color = theme.GetUIColor(ColorAlias.Black, 0xFF),
                Size = new Size(5, 5),
            });
            inkCanvas.InkPresenter.StrokesCollected += StrokesCollected;
            inkCanvas.InkPresenter.StrokesErased += StrokesErased;
        }

        private async void SignOutClicked(object sender, RoutedEventArgs e) {
            ContentDialog dialog = new ContentDialog {
                Content = "Signing out removes your account from this device. It will not affect your files, but if you haven't backed up your private key or signed in on another device, you will forever lose access to your account.",
                Title = "Confirm Sign Out",
                IsSecondaryButtonEnabled = true,
                PrimaryButtonText = "Remove Account From This Device",
                SecondaryButtonText = "Cancel",
            };

            var bst = new Style(typeof(Button));
            bst.Setters.Add(new Setter(BackgroundProperty, Colors.Red));
            bst.Setters.Add(new Setter(ForegroundProperty, Colors.White));
            dialog.PrimaryButtonStyle = bst;

            if (await dialog.ShowAsync() == ContentDialogResult.Primary) {
                await App.SignOut();
            }
        }

        private async void NavigationViewLoaded(object sender, RoutedEventArgs e) {
            await ReloadFiles();
            CheckForWorkLoop();
        }

        private async void CheckForWorkLoop() {
            while (true) {
                await ReloadCalculatedWork();
                await Task.Delay(2000);
            }
        }

        public async Task ReloadCalculatedWork() {
            switch (await App.CoreService.CalculateWork()) {
                case Core.CalculateWork.Success success:
                    App.IsOnline = true;
                    itemsToSync = success.workCalculated.workUnits.Count;
                    break;
                case Core.CalculateWork.UnexpectedError uhOh:
                    System.Diagnostics.Debug.WriteLine("Unexpected error during calc work loop: " + uhOh.ErrorMessage);
                    break;
                case Core.CalculateWork.ExpectedError error:
                    switch (error.Error) {
                        case Core.CalculateWork.PossibleErrors.CouldNotReachServer:
                            App.IsOnline = false;
                            break;
                        case Core.CalculateWork.PossibleErrors.ClientUpdateRequired:
                            App.ClientUpdateRequired = true;
                            App.Refresh();
                            break;
                        case Core.CalculateWork.PossibleErrors.NoAccount:
                            await App.ReloadDbStateAndAccount();
                            break;
                    }
                    break;
            }
        }

        public void Refresh() {
            switch (currentEditMode) {
                case EditMode.None:
                    textEditor.Visibility = Visibility.Collapsed;
                    drawEditor.Visibility = Visibility.Collapsed;
                    break;
                case EditMode.Text:
                    textEditor.Visibility = Visibility.Visible;
                    drawEditor.Visibility = Visibility.Collapsed;
                    break;
                case EditMode.Draw:
                    textEditor.Visibility = Visibility.Collapsed;
                    drawEditor.Visibility = Visibility.Visible;
                    break;
            }
            if (!App.IsOnline) {
                syncIcon.Glyph = offlineGlyph;
                syncText.Text = "Offline";
            }
            if (SyncWorking) {
                syncIcon.Glyph = syncGlyph;
                syncText.Text = "Syncing...";
            }
            if (ItemsToSync == 0) {
                syncIcon.Glyph = checkGlyph;
                syncText.Text = "Up to date";
            } else if (ItemsToSync == 1) {
                syncIcon.Glyph = syncGlyph;
                syncText.Text = ItemsToSync + " item need to be synced";
            } else {
                syncIcon.Glyph = syncGlyph;
                syncText.Text = ItemsToSync + " items need to be synced";
            }
        }

        private async Task ReloadFiles() {
            switch (await App.CoreService.ListMetadatas()) {
                case Core.ListMetadatas.Success success:
                    await PopulateTree(success.files);
                    break;
                case Core.ListMetadatas.UnexpectedError ohNo:
                    await new MessageDialog(ohNo.ErrorMessage, "Unexpected Error!").ShowAsync();
                    break;
            }
        }

        private async Task PopulateTree(List<FileMetadata> files) {
            files = files.Where(f => !f.deleted).ToList();
            var newUIFiles = new Dictionary<string, UIFile>();
            var root = files.FirstOrDefault(file => file.Id == file.Parent);
            if (root == null) {
                await new MessageDialog("Root not found, file a bug report!", "Root not found!").ShowAsync();
                return;
            }
            PopulateTreeRecursive(files, newUIFiles, root);
            newUIFiles[root.Id].IsRoot = true;
            foreach (var f in App.UIFiles) {
                if (f.Value.IsExpanded) {
                    if (newUIFiles.TryGetValue(f.Key, out var newUIFile)) {
                        newUIFile.IsExpanded = true;
                    }
                }
            }
            Files.Clear();
            Files.Add(newUIFiles[root.Id]);
            App.UIFiles = newUIFiles;
        }

        private void PopulateTreeRecursive(List<FileMetadata> files, Dictionary<string, UIFile> tree, FileMetadata file) {
            tree[file.Id] = new UIFile {
                Id = file.Id,
                Name = file.Name,
                IsDocument = file.Type == "Document",
                Children = file.Type == "Document" ? null : new ObservableCollection<UIFile>(),
            };
            if (file.Id != file.Parent) {
                tree[file.Parent].Children.Add(tree[file.Id]);
            }
            foreach (var f in files.Where(f => f.Parent == file.Id && f.Id != file.Id)) {
                PopulateTreeRecursive(files, tree, f);
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
                    await ReloadFiles();
                    break;
                case Core.CreateFile.UnexpectedError uhOh:
                    await new MessageDialog(uhOh.ErrorMessage, "Unexpected Error!").ShowAsync();
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
            SyncWorking = true;
            switch (await App.CoreService.SyncAll()) {
                case Core.SyncAll.Success:
                    App.IsOnline = true;
                    await ReloadFiles();
                    await ReloadCalculatedWork();
                    break;
                case Core.SyncAll.UnexpectedError uhOh:
                    await new MessageDialog(uhOh.ErrorMessage, "Unexpected Error!").ShowAsync();
                    break;
                case Core.SyncAll.ExpectedError error:
                    switch (error.Error) {
                        case Core.SyncAll.PossibleErrors.CouldNotReachServer:
                            App.IsOnline = false;
                            break;
                        case Core.SyncAll.PossibleErrors.ClientUpdateRequired:
                            App.ClientUpdateRequired = true;
                            App.Refresh();
                            break;
                        case Core.SyncAll.PossibleErrors.NoAccount:
                            await App.ReloadDbStateAndAccount();
                            break;
                        case Core.SyncAll.PossibleErrors.ExecuteWorkError:
                            await new MessageDialog(error.ToString(), "Unexpected Error!").ShowAsync();
                            break;
                    }
                    break;
            }
            SyncWorking = false;
        }

        private async void RenameFile(object sender, RoutedEventArgs e) {
            string id = (string)((MenuFlyoutItem)sender).Tag;
            string newName = await InputTextDialogAsync("Choose a new name");

            var result = await App.CoreService.RenameFile(id, newName);

            switch (result) {
                case Core.RenameFile.Success:
                    await ReloadFiles();
                    // todo: re-open file
                    break;
                case Core.RenameFile.UnexpectedError uhOh:
                    await new MessageDialog(uhOh.ErrorMessage, "Unexpected Error!").ShowAsync();
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
            }
        }

        private async void DeleteFile(object sender, RoutedEventArgs e) {
            string id = (string)((MenuFlyoutItem)sender).Tag;

            var result = await App.CoreService.DeleteFile(id);

            switch (result) {
                case Core.DeleteFile.Success:
                    await ReloadFiles();
                    break;
                case Core.DeleteFile.UnexpectedError uhOh:
                    await new MessageDialog(uhOh.ErrorMessage, "Unexpected Error!").ShowAsync();
                    break;
                case Core.DeleteFile.ExpectedError error:
                    switch (error.Error) {
                        case Core.DeleteFile.PossibleErrors.FileDoesNotExist:
                            await new MessageDialog("Could not locate the file you're trying to delete! Please file a bug report.", "Unexpected Error!").ShowAsync();
                            break;
                        case Core.DeleteFile.PossibleErrors.CannotDeleteRoot:
                            await new MessageDialog("You cannot delete your root folder!", "Delete Error!").ShowAsync();
                            break;
                    }
                    break;
            }
        }

        private void NavigationViewItem_DragOver(object sender, DragEventArgs e) {
            e.AcceptedOperation = DataPackageOperation.Move; // TODO show none over documents
        }

        private async void NavigationViewItem_Drop(object sender, Microsoft.UI.Xaml.Controls.TreeViewDragItemsCompletedEventArgs e) {
            var toMove = (e.Items[0] as UIFile)?.Id;
            var newParent = (e.NewParentItem as UIFile)?.Id;
            if (toMove == null || newParent == null) {
                return;
            }

            App.UIFiles.TryGetValue(toMove, out var toMoveFile);
            App.UIFiles.TryGetValue(newParent, out var newParentFile);
            if (toMoveFile.IsRoot || toMove == newParent || newParentFile.IsDocument || newParentFile.Children.Contains(toMoveFile)) {
                return;
            }

            var result = await App.CoreService.MoveFile(toMove, newParent);

            switch (result) {
                case Core.MoveFile.Success:
                    break;
                case Core.MoveFile.UnexpectedError uhOh:
                    await new MessageDialog(uhOh.ErrorMessage, "Unexpected Error!").ShowAsync();
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
                        case Core.MoveFile.PossibleErrors.FolderMovedIntoItself:
                            await new MessageDialog("Cannot move parent into a child folder!", "Move failed!").ShowAsync();
                            break;
                    }
                    break;
            }

            await ReloadFiles();
        }

        private async void DocumentSelected(object sender, Windows.UI.Xaml.Input.TappedRoutedEventArgs e) {
            string tag = (string)((FrameworkElement)sender).Tag;
            var file = App.UIFiles[tag];

            inkCanvas.InkPresenter.StrokeContainer.Clear();
            textEditor.TextDocument.SetText(TextSetOptions.None, "");

            if (file.IsDocument) {
                SelectedDocumentId = tag;
                var result = await App.CoreService.ReadDocument(SelectedDocumentId);

                switch (result) {
                    case Core.ReadDocument.Success content:
                        if (file.Name.EndsWith(".draw")) {
                            CurrentEditMode = EditMode.Draw;
                            if (!string.IsNullOrWhiteSpace(content.content)) {
                                drawingContext = JsonConvert.DeserializeObject<Drawing>(content.content).GetContext();
                            } else {
                                drawingContext = new DrawingContext();
                            }
                            foreach (var stroke in drawingContext.strokes) {
                                inkCanvas.InkPresenter.StrokeContainer.AddStroke(stroke);
                            }
                            PenPaletteBlack.Color = theme.GetUIColor(ColorAlias.Black, 1f);
                            PenPaletteRed.Color = theme.GetUIColor(ColorAlias.Red, 1f);
                            PenPaletteGreen.Color = theme.GetUIColor(ColorAlias.Green, 1f);
                            PenPaletteBlue.Color = theme.GetUIColor(ColorAlias.Blue, 1f);
                            PenPaletteYellow.Color = theme.GetUIColor(ColorAlias.Yellow, 1f);
                            PenPaletteMagenta.Color = theme.GetUIColor(ColorAlias.Magenta, 1f);
                            PenPaletteCyan.Color = theme.GetUIColor(ColorAlias.Cyan, 1f);
                            PenPaletteWhite.Color = theme.GetUIColor(ColorAlias.White, 1f);
                        } else {
                            CurrentEditMode = EditMode.Text;
                            textEditor.TextDocument.SetText(TextSetOptions.None, content.content);
                            textEditor.TextDocument.ClearUndoRedoHistory();
                        }
                        editCount[tag] = 0;
                        break;
                    case Core.ReadDocument.UnexpectedError uhOh:
                        await new MessageDialog(uhOh.ErrorMessage, "Unexpected Error!").ShowAsync();
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
                }
            } else {
                CurrentEditMode = EditMode.None;
            }
        }

        private async void TextChanged(object sender, RoutedEventArgs e) {
            if (SelectedDocumentId != "" && textEditor.FocusState != FocusState.Unfocused) {
                // Capture variables in case document is closed before save actually happens
                string docID = SelectedDocumentId;
                string text;
                textEditor.TextDocument.GetText(TextGetOptions.UseLf, out text);

                // Only save the document if no keystrokes have happened in the last .5 seconds
                editCount[docID]++;
                var current = editCount[docID];
                await Task.Delay(500);
                if (current != editCount[docID]) {
                    return;
                }

                await SaveContent(docID, text);
            }
        }

        private void StrokesErased(InkPresenter sender, InkStrokesErasedEventArgs args) {
            var redraw = false;
            foreach (var erasedStroke in args.Strokes) {
                var removedCount = 0;
                foreach(var connectedSubstroke in drawingContext.splitStrokes[erasedStroke]) {
                    drawingContext.strokes.Remove(connectedSubstroke);
                    drawingContext.splitStrokes.Remove(connectedSubstroke);
                    removedCount++;
                }
                if(removedCount > 1) {
                    redraw = true;
                }
            }

            // todo: better way to erase
            if(redraw) {
                inkCanvas.InkPresenter.StrokeContainer.Clear();
                foreach (var stroke in drawingContext.GetDrawing().GetContext().strokes) {
                    inkCanvas.InkPresenter.StrokeContainer.AddStroke(stroke);
                }
            }

            DrawingChanged();
        }

        private void StrokesCollected(InkPresenter sender, InkStrokesCollectedEventArgs args) {
            foreach (var addedStroke in args.Strokes) {
                drawingContext.strokes.Add(addedStroke);
                drawingContext.splitStrokes[addedStroke] = new List<InkStroke> { addedStroke };
            }

            DrawingChanged();
        }

        private async void DrawingChanged() {
            // Capture variables in case document is closed before save actually happens
            string docID = SelectedDocumentId;
            var context = drawingContext;

            // Only save the document if no strokes have happened in the last .5 seconds
            editCount[docID]++;
            var current = editCount[docID];
            await Task.Delay(500);
            if (current != editCount[docID]) {
                return;
            }

            var drawingJSON = JsonConvert.SerializeObject(context.GetDrawing());
            await SaveContent(docID, drawingJSON);
        }

        private async Task SaveContent(string docID, string text) {
            var result = await App.CoreService.WriteDocument(docID, text);

            switch (result) {
                case Core.WriteDocument.Success:
                    await ReloadCalculatedWork();
                    break;
                case Core.WriteDocument.UnexpectedError uhOh:
                    await new MessageDialog(uhOh.ErrorMessage, "Unexpected Error!").ShowAsync();
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
            }
        }

        private async void ListViewItem_Tapped(object sender, Windows.UI.Xaml.Input.TappedRoutedEventArgs e) {
            SignInContentDialog signInDialog = new SignInContentDialog();
            await signInDialog.ShowAsync();
        }

        DateTime prev;
        private void Pasted(object sender, TextControlPasteEventArgs e) {
            var now = DateTime.Now;
            if (now - prev < TimeSpan.FromMilliseconds(10)) {
                e.Handled = true;
            } else {
                prev = now;
            }
        }

        private void Undo(object sender, RoutedEventArgs e) {
            // todo
        }

        private void Redo(object sender, RoutedEventArgs e) {
            // todo
        }

        private void ToggleTouchDrawing(object sender, RoutedEventArgs e) {
            if (toggleButton.IsChecked == true) {
                inkCanvas.InkPresenter.InputDeviceTypes |= CoreInputDeviceTypes.Touch;
            } else {
                inkCanvas.InkPresenter.InputDeviceTypes &= ~CoreInputDeviceTypes.Touch;
            }
        }
    }
}
