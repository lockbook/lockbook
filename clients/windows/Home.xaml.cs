using System.Collections.ObjectModel;
using System.Linq;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;

namespace lockbook
{
    public sealed partial class Home : Page
    {
        public ObservableCollection<File> Files;
        int n;

        public Home()
        {
            InitializeComponent();
            Files = new ObservableCollection<File>();
        }

        private void AddFile(object sender, RoutedEventArgs e)
        {
            Files.Add(new File { Path = "this/is/a/file/path/" + n, Content = "This is file content " + n + "."});
            n += Core.X();
        }

        private void FileSelected(object sender, SelectionChangedEventArgs e)
        {
            var selected = (File)e.AddedItems.Single();
            FileContentTextBlock.Text = selected.Content;
        }
    }

    public class File
    {
        public string Path { get; set; }
        public string Content { get; set; }
    }
}
