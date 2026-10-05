using System;
using System.IO;
using System.Runtime.InteropServices;

// Measures ERROR_ELEVATION_REQUIRED (740) honestly: this program calls
// CreateProcessW on a requireAdministrator image from inside its own process,
// so the elevation check runs against this process's own token rather than
// against a token handed to CreateProcessAsUser. Staging it under a restricted
// Medium token and reading the number it reports is what makes the boundary
// measurable without a UAC prompt or a second host.
//
// argv[0] = path of the requireAdministrator image to attempt
// argv[1] = path of the result file to write
//
// The result file carries the caller's integrity SID and the raw Win32 error,
// and nothing else - no paths, no process ids, no message text.
internal static class ElevationRequiredProbe
{
    [StructLayout(LayoutKind.Sequential)]
    private struct ProcessInformation
    {
        public IntPtr Process;
        public IntPtr Thread;
        public int ProcessId;
        public int ThreadId;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct StartupInfo
    {
        public int Cb;
        public IntPtr Reserved;
        public IntPtr Desktop;
        public IntPtr Title;
        public int X;
        public int Y;
        public int XSize;
        public int YSize;
        public int XCountChars;
        public int YCountChars;
        public int FillAttribute;
        public int Flags;
        public short ShowWindow;
        public short Reserved2;
        public IntPtr Reserved3;
        public IntPtr StdInput;
        public IntPtr StdOutput;
        public IntPtr StdError;
    }

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool CreateProcessW(
        string applicationName,
        string commandLine,
        IntPtr processAttributes,
        IntPtr threadAttributes,
        bool inheritHandles,
        uint creationFlags,
        IntPtr environment,
        string currentDirectory,
        ref StartupInfo startupInfo,
        out ProcessInformation processInformation);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool TerminateProcess(IntPtr process, uint exitCode);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool CloseHandle(IntPtr handle);

    private static int Main(string[] args)
    {
        if (args.Length < 2)
        {
            return 2;
        }

        string target = args[0];
        string resultPath = args[1];

        string integritySid = "unknown";
        try
        {
            var identity = System.Security.Principal.WindowsIdentity.GetCurrent();
            foreach (var group in identity.Groups)
            {
                string value = group.Value;
                if (value != null && value.StartsWith("S-1-16-", StringComparison.Ordinal))
                {
                    integritySid = value;
                    break;
                }
            }
        }
        catch
        {
            integritySid = "unreadable";
        }

        var startup = new StartupInfo();
        startup.Cb = Marshal.SizeOf(typeof(StartupInfo));
        ProcessInformation info;

        bool launched = CreateProcessW(target, null, IntPtr.Zero, IntPtr.Zero, false, 0, IntPtr.Zero, null, ref startup, out info);

        // GetLastError is only meaningful when the call failed. Reporting whatever
        // happened to be left in it after a success reads as a failure code that
        // never occurred, which is exactly the confusion this probe exists to avoid.
        int lastError = launched ? 0 : Marshal.GetLastWin32Error();

        if (launched)
        {
            try
            {
                TerminateProcess(info.Process, 0);
            }
            catch
            {
            }

            CloseHandle(info.Process);
            CloseHandle(info.Thread);
        }

        string line = "integrity_sid=" + integritySid
            + " launched=" + (launched ? "1" : "0")
            + " win32_error=" + lastError.ToString(System.Globalization.CultureInfo.InvariantCulture);

        try
        {
            File.WriteAllText(resultPath, line);
        }
        catch
        {
            return 3;
        }

        return 0;
    }
}
