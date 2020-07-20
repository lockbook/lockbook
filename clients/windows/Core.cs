using System;
using System.Diagnostics;
using System.Runtime.InteropServices;

namespace lockbook
{
    class Core
    {
        [DllImport("lockbook_core.dll")]
        public static extern IntPtr return_string();

        [DllImport("lockbook_core.dll")]
        public static extern void accept_string([MarshalAs(UnmanagedType.LPStr)] string s);

        [DllImport("lockbook_core.dll")]
        public static extern IntPtr echo_string([MarshalAs(UnmanagedType.LPStr)] string s);

        public static int X()
        {
            accept_string("hello from c#!");
            Debug.WriteLine(Marshal.PtrToStringAnsi(return_string()));
            Debug.WriteLine(Marshal.PtrToStringAnsi(echo_string("beware the babayaga")));

            return 1;
        }
    }
}
