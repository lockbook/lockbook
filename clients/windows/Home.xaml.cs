using System.Collections.ObjectModel;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;

// The Blank Page item template is documented at https://go.microsoft.com/fwlink/?LinkId=402352&clcid=0x409

namespace lockbook
{
    /// <summary>
    /// An empty page that can be used on its own or navigated to within a Frame.
    /// </summary>
    public sealed partial class Home : Page
    {
        public ObservableCollection<File> Files;

        public Home()
        {
            InitializeComponent();
            Files = new ObservableCollection<File>();
        }

        private void Button_Click(object sender, RoutedEventArgs e)
        {
            Files.Add(new File { Id = "this is a file id" });
        }
    }

    public class File
    {
        public string Id { get; set; }
    }
}
