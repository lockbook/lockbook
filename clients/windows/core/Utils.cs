using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;

namespace core {
    static class Utils {
        const byte CR = 0x0D;
        const byte LF = 0x0A;

        // remember to Marshal.FreeHGlobal(result) when you're done!
        public static IntPtr ToFFI(string str) {
            var bytes = Encoding.UTF8.GetBytes(str).ConvertLineEndingsToUnix().AddNullTerminator().ToArray();
            var result = Marshal.AllocHGlobal(bytes.Length);
            Marshal.Copy(bytes, 0, result, bytes.Length);
            return result;
        }

        public static string FromFFI(IntPtr ptr) {
            return Encoding.UTF8.GetString(ReadBytes(ptr).ConvertLineEndingsToDOS().ToArray());
        }

        private static IEnumerable<byte> ReadBytes(IntPtr ptr) {
            for (var i = 0; ; i++) {
                var b = Marshal.ReadByte(ptr, i);
                if (b == 0) {
                    yield break; // read until null terminator
                }
                yield return b;
            }
        }

        private static IEnumerable<byte> AddNullTerminator(this IEnumerable<byte> bytes) {
            var e = bytes.GetEnumerator();
            while (e.MoveNext()) {
                yield return e.Current;
            }
            yield return 0; // add null terminator
        }

        private static IEnumerable<byte> ConvertLineEndingsToDOS(this IEnumerable<byte> bytes) {
            var e = bytes.GetEnumerator();
            while (e.MoveNext()) {
                if (e.Current == LF) { // LF -> {CR, LF}
                    yield return CR;
                    yield return LF;
                } else {
                    yield return e.Current;
                }
            }
        }

        private static IEnumerable<byte> ConvertLineEndingsToUnix(this IEnumerable<byte> bytes) {
            var e = bytes.GetEnumerator();
            while (e.MoveNext()) {
                if (e.Current != CR) { // CR -> {}
                    yield return e.Current;
                }
            }
        }
    }
}
