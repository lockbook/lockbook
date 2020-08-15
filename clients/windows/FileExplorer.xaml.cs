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
                    await PopulateTree(success.files);
                    break;
                case Core.ListFileMetadata.UnexpectedError ohNo:
                    await new MessageDialog(ohNo.errorMessage, "Unexpected Error!").ShowAsync();
                    break;
            }

        }

        private async Task PopulateTree(List<FileMetadata> coreFiles) {
            FileMetadata root = null;
            Dictionary<String, UIFile> uiFiles = new Dictionary<string, UIFile>();

            // Find our root
            foreach (var file in coreFiles) {
                if (file.Id == file.Parent) {
                    root = file;
                }
            }

            if (root == null) {
                await new MessageDialog("Root not found, file a bug report!", "Root not found!").ShowAsync();
            }

            Queue<FileMetadata> toExplore = new Queue<FileMetadata>();
            uiFiles[root.Id] = new UIFile { Id = root.Id, Name = root.Name, Children = new ObservableCollection<UIFile>() };
            toExplore.Enqueue(root);
            Files.Add(uiFiles[root.Id]);

            while (toExplore.Count != 0) {
                var current = toExplore.Dequeue();
                
                // Find all children
                foreach (var file in coreFiles) {
                    if (current.Id == file.Parent && file.Parent != file.Id) {
                        toExplore.Enqueue(file);
                        var newUi = new UIFile { Name = file.Name, Id = file.Id, Children = new ObservableCollection<UIFile>() };
                        uiFiles[file.Id] = newUi;
                        uiFiles[current.Id].Children.Add(newUi);
                    }
                }
            }

            
        }
    }

}
