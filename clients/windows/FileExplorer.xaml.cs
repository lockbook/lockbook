using Microsoft.UI.Xaml.Controls;
using System;
using System.Collections.ObjectModel;
using System.Collections.Specialized;
using System.ComponentModel;
using Windows.ApplicationModel.Core;
using Windows.Storage;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;

namespace lockbook {
    public class File : INotifyPropertyChanged {
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

        public ObservableCollection<File> Children { get; set; }
    }


    public sealed partial class FileExplorer : Page {

        public ObservableCollection<File> Categories = new ObservableCollection<File>() {
            new File(){
                Name = "Menu item 1",
                Id = "id1",
                Icon = "Icon",
                Expanded = true,
                Children = new ObservableCollection<File>() {
                    new File(){
                        Name = "Menu item 2",
                        Icon = "Icon",
                        Id = "id2",

                        Expanded = true,

                        Children = new ObservableCollection<File>() {
                            new File() {
                                Name  = "Menu item 3",
                                Id = "id3",
                                Icon = "Icon",
                                Expanded = false,
                                Children = new ObservableCollection<File>() {
                                    new File() { Name  = "Menu item 4", Icon = "Icon", Id = "id4", Expanded=false },
                                    new File() { Name  = "Menu item 5", Icon = "Icon", Id = "id5", Expanded=false }
                                }
                            }
                        }
                    }
                }
            },

        };

        public FileExplorer() {
            InitializeComponent();
            System.Diagnostics.Debug.WriteLine(Categories[0].Expanded);
        }


        private async void ClearStateClicked(object sender, RoutedEventArgs e) {
            await ApplicationData.Current.ClearAsync();
            CoreApplication.Exit();
        }


        private void OnItemInvoked(object sender, Microsoft.UI.Xaml.Controls.NavigationViewItemInvokedEventArgs e) {
            var clickedItem = e.InvokedItem;
            var clickedItemContainer = e.InvokedItemContainer;
            Categories[0].Expanded = false;
            System.Diagnostics.Debug.WriteLine(Categories[0].Expanded);

        }
        private void OnItemExpanding(object sender, NavigationViewItemExpandingEventArgs e) {
            var nvib = e.ExpandingItemContainer;
            System.Diagnostics.Debug.WriteLine(Categories[0].Expanded);
        }
        private void OnItemCollapsed(object sender, NavigationViewItemCollapsedEventArgs e) {
            var nvib = e.CollapsedItemContainer;
            var name = "Last collapsed: " + nvib.Content;
            System.Diagnostics.Debug.WriteLine(Categories[0].Expanded);


        }
    }

}
