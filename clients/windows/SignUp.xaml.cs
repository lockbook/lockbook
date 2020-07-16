using System.Numerics;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;

// The Blank Page item template is documented at https://go.microsoft.com/fwlink/?LinkId=234238

namespace lockbook
{
    /// <summary>
    /// An empty page that can be used on its own or navigated to within a Frame.
    /// </summary>
    public sealed partial class SignUp : Page
    {
        public SignUp()
        {
            InitializeComponent();
        }

        private void CreateAccount(object sender, RoutedEventArgs e)
        {
            Frame.Navigate(typeof(Home));
        }

        private void ImportAccount(object sender, RoutedEventArgs e)
        {
            Frame.Navigate(typeof(Home));
        }
    }
}
