using System;
using System.Diagnostics;
using System.Runtime.InteropServices;

namespace lockbook
{
    class Core
    {
        [DllImport("lockbook_core.dll")]
        static extern void init_logger();
    }
}
