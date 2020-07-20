using System;
using System.Diagnostics;
using System.Runtime.InteropServices;

namespace lockbook
{
    class Core
    {
        [StructLayout(LayoutKind.Sequential)]
        struct RustResultWrapper
        {
            bool is_error;
            RustValue value;
            RustLockbookError error;
        }

        [StructLayout(LayoutKind.Explicit)]
        struct RustValue
        {
            [FieldOffset(0)]
            [MarshalAs(UnmanagedType.LPStr)]
            public string success;

            [FieldOffset(0)]
            [MarshalAs(UnmanagedType.LPStr)]
            public string error;
        }

        enum RustLockbookError
        {
            Network,
            Database,
        }

        [DllImport("lockbook_core.dll")]
        static extern void init_logger();

        [DllImport("lockbook_core.dll")]
        [return: MarshalAs(UnmanagedType.I1)]
        static extern bool is_db_present(
            [MarshalAs(UnmanagedType.LPStr)] string path);

        [DllImport("lockbook_core.dll")]
        static extern void release_pointer(
            [MarshalAs(UnmanagedType.LPStr)] string s);

        [DllImport("lockbook_core.dll")]
        static extern RustResultWrapper get_account(
            [MarshalAs(UnmanagedType.LPStr)] string path);

        [DllImport("lockbook_core.dll")]
        static extern RustResultWrapper create_account(
            [MarshalAs(UnmanagedType.LPStr)] string path,
            [MarshalAs(UnmanagedType.LPStr)] string username);

        [DllImport("lockbook_core.dll")]
        static extern RustResultWrapper import_account(
            [MarshalAs(UnmanagedType.LPStr)] string path,
            [MarshalAs(UnmanagedType.LPStr)] string account);

        [DllImport("lockbook_core.dll")]
        static extern RustResultWrapper sync_files(
            [MarshalAs(UnmanagedType.LPStr)] string path);

        [DllImport("lockbook_core.dll")]
        static extern RustResultWrapper calculate_work(
            [MarshalAs(UnmanagedType.LPStr)] string path);

        [DllImport("lockbook_core.dll")]
        static extern RustResultWrapper execute_work(
            [MarshalAs(UnmanagedType.LPStr)] string path,
            [MarshalAs(UnmanagedType.LPStr)] string work_unit);

        [DllImport("lockbook_core.dll")]
        static extern RustResultWrapper get_root(
            [MarshalAs(UnmanagedType.LPStr)] string path);

        [DllImport("lockbook_core.dll")]
        static extern RustResultWrapper list_files(
            [MarshalAs(UnmanagedType.LPStr)] string path,
            [MarshalAs(UnmanagedType.LPStr)] string parent_id);

        [DllImport("lockbook_core.dll")]
        static extern RustResultWrapper get_file(
            [MarshalAs(UnmanagedType.LPStr)] string path,
            [MarshalAs(UnmanagedType.LPStr)] string file_id);

        [DllImport("lockbook_core.dll")]
        static extern RustResultWrapper create_file(
            [MarshalAs(UnmanagedType.LPStr)] string path,
            [MarshalAs(UnmanagedType.LPStr)] string file_name,
            [MarshalAs(UnmanagedType.LPStr)] string file_parent_id,
            [MarshalAs(UnmanagedType.I1)] bool is_folder);

        [DllImport("lockbook_core.dll")]
        static extern RustResultWrapper update_file(
            [MarshalAs(UnmanagedType.LPStr)] string path,
            [MarshalAs(UnmanagedType.LPStr)] string file_id,
            [MarshalAs(UnmanagedType.LPStr)] string file_content);

        [DllImport("lockbook_core.dll")]
        static extern RustResultWrapper mark_file_for_deletion(
            [MarshalAs(UnmanagedType.LPStr)] string path,
            [MarshalAs(UnmanagedType.LPStr)] string file_id);

        public static int X()
        {
            var path = Windows.Storage.ApplicationData.Current.LocalFolder.Path;
            Debug.WriteLine(is_db_present(path));

            return 1;
        }
    }
}
