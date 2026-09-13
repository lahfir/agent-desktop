using System;
using System.ComponentModel;
using System.Runtime.InteropServices;

namespace AgentDesktopProbe {

    public class HandleResult {
        public long Handle;
        public bool Success;
        public int LastError;
        public string ErrorName;
        public string Message;
    }

    public class OpResult {
        public bool Success;
        public int LastError;
        public string ErrorName;
        public string Message;
        public string Detail;
    }

    public class RemoteProtocolResult {
        public string Target;
        public bool OpenSuccess;
        public int OpenError;
        public string OpenErrorName;
        public bool ControlCallSuccess;
        public int ControlLastError;
        public string ControlErrorName;
        public string ControlDetail;
        public int InfoClass;
        public bool OutOfRangeClassCallSuccess;
        public int OutOfRangeClassLastError;
        public string OutOfRangeClassErrorName;
        public bool CallSuccess;
        public int LastError;
        public string ErrorName;
        public string Message;
        public int StructureVersion;
        public int StructureSize;
        public string Protocol;
        public string ProtocolName;
        public int ProtocolMajorVersion;
        public int ProtocolMinorVersion;
        public int ProtocolRevision;
        public string Flags;
        public string FlagNames;
        public int DriveType;
        public string DriveTypeName;
        public bool PathIsNetworkPath;
    }

    public static class PrivateFile {

        private const uint OPEN_EXISTING = 3;
        private const uint FILE_FLAG_BACKUP_SEMANTICS = 0x02000000;
        private const int FileBasicInfo = 0;
        private const int FileRemoteProtocolInfo = 13;
        private const int OutOfRangeInfoClass = 55;
        private static readonly IntPtr InvalidHandle = new IntPtr(-1);

        [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        private static extern IntPtr CreateFileW(string lpFileName, uint dwDesiredAccess, uint dwShareMode,
            IntPtr lpSecurityAttributes, uint dwCreationDisposition, uint dwFlagsAndAttributes, IntPtr hTemplateFile);

        [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        private static extern bool MoveFileExW(string lpExistingFileName, string lpNewFileName, uint dwFlags);

        [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        private static extern bool ReplaceFileW(string lpReplacedFileName, string lpReplacementFileName,
            string lpBackupFileName, uint dwReplaceFlags, IntPtr lpExclude, IntPtr lpReserved);

        [DllImport("kernel32.dll", SetLastError = true)]
        private static extern bool GetFileInformationByHandleEx(IntPtr hFile, int FileInformationClass,
            IntPtr lpFileInformation, uint dwBufferSize);

        [DllImport("kernel32.dll", SetLastError = true)]
        private static extern bool ReadFile(IntPtr hFile, byte[] lpBuffer, uint nNumberOfBytesToRead,
            out uint lpNumberOfBytesRead, IntPtr lpOverlapped);

        [DllImport("kernel32.dll", SetLastError = true)]
        private static extern uint SetFilePointer(IntPtr hFile, int lDistanceToMove, IntPtr lpDistanceToMoveHigh, uint dwMoveMethod);

        [DllImport("kernel32.dll", SetLastError = true)]
        private static extern bool CloseHandle(IntPtr hObject);

        [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        private static extern uint GetDriveTypeW(string lpRootPathName);

        [DllImport("shlwapi.dll", SetLastError = false, CharSet = CharSet.Unicode)]
        private static extern bool PathIsNetworkPathW(string pszPath);

        public static string ErrorName(int code) {
            switch (code) {
                case 0: return "ERROR_SUCCESS";
                case 2: return "ERROR_FILE_NOT_FOUND";
                case 3: return "ERROR_PATH_NOT_FOUND";
                case 5: return "ERROR_ACCESS_DENIED";
                case 6: return "ERROR_INVALID_HANDLE";
                case 19: return "ERROR_WRITE_PROTECT";
                case 32: return "ERROR_SHARING_VIOLATION";
                case 33: return "ERROR_LOCK_VIOLATION";
                case 50: return "ERROR_NOT_SUPPORTED";
                case 53: return "ERROR_BAD_NETPATH";
                case 80: return "ERROR_FILE_EXISTS";
                case 87: return "ERROR_INVALID_PARAMETER";
                case 183: return "ERROR_ALREADY_EXISTS";
                case 995: return "ERROR_OPERATION_ABORTED";
                case 1005: return "ERROR_UNRECOGNIZED_VOLUME";
                case 1176: return "ERROR_UNABLE_TO_MOVE_REPLACEMENT";
                case 1177: return "ERROR_UNABLE_TO_MOVE_REPLACEMENT_2";
                case 1178: return "ERROR_UNABLE_TO_REMOVE_REPLACED";
                case 4390: return "ERROR_NOT_A_REPARSE_POINT";
                default: return "ERROR_" + code;
            }
        }

        private static string SystemMessage(int code) {
            try {
                return new Win32Exception(code).Message;
            } catch (Exception) {
                return "";
            }
        }

        private static string DriveTypeName(uint value) {
            switch (value) {
                case 0: return "DRIVE_UNKNOWN";
                case 1: return "DRIVE_NO_ROOT_DIR";
                case 2: return "DRIVE_REMOVABLE";
                case 3: return "DRIVE_FIXED";
                case 4: return "DRIVE_REMOTE";
                case 5: return "DRIVE_CDROM";
                case 6: return "DRIVE_RAMDISK";
                default: return "DRIVE_" + value;
            }
        }

        private static string ProtocolName(uint value) {
            switch (value) {
                case 0x00020000: return "WNNC_NET_SMB";
                case 0x00420000: return "WNNC_NET_MS_NFS";
                case 0x00430000: return "WNNC_NET_MS_DAV";
                case 0x00000000: return "WNNC_NET_NONE";
                default: return "0x" + value.ToString("X8");
            }
        }

        private static string FlagNames(uint value) {
            string names = "";
            if ((value & 0x00000001) != 0) { names = names + "LOOPBACK|"; }
            if ((value & 0x00000002) != 0) { names = names + "OFFLINE|"; }
            if ((value & 0x00000004) != 0) { names = names + "PERSISTENT_HANDLE|"; }
            if ((value & 0x00000008) != 0) { names = names + "PRIVACY|"; }
            if ((value & 0x00000010) != 0) { names = names + "INTEGRITY|"; }
            if ((value & 0x00000020) != 0) { names = names + "MUTUAL_AUTH|"; }
            if (names.Length == 0) { return "NONE"; }
            return names.Substring(0, names.Length - 1);
        }

        public static HandleResult OpenHandle(string path, uint access, uint share, uint flags) {
            HandleResult result = new HandleResult();
            IntPtr h = CreateFileW(path, access, share, IntPtr.Zero, OPEN_EXISTING, flags, IntPtr.Zero);
            int err = Marshal.GetLastWin32Error();
            if (h == InvalidHandle) {
                result.Handle = -1;
                result.Success = false;
                result.LastError = err;
                result.ErrorName = ErrorName(err);
                result.Message = SystemMessage(err);
                return result;
            }
            result.Handle = h.ToInt64();
            result.Success = true;
            result.LastError = 0;
            result.ErrorName = ErrorName(0);
            result.Message = "";
            return result;
        }

        public static bool CloseFileHandle(long handle) {
            if (handle == 0 || handle == -1) { return false; }
            return CloseHandle(new IntPtr(handle));
        }

        public static OpResult ReadFromHandle(long handle, int count) {
            OpResult result = new OpResult();
            IntPtr h = new IntPtr(handle);
            SetFilePointer(h, 0, IntPtr.Zero, 0);
            byte[] buffer = new byte[count];
            uint read = 0;
            bool ok = ReadFile(h, buffer, (uint)count, out read, IntPtr.Zero);
            int err = Marshal.GetLastWin32Error();
            result.Success = ok;
            result.LastError = ok ? 0 : err;
            result.ErrorName = ErrorName(ok ? 0 : err);
            result.Message = ok ? "" : SystemMessage(err);
            result.Detail = ok ? System.Text.Encoding.ASCII.GetString(buffer, 0, (int)read) : "";
            return result;
        }

        public static OpResult MoveOver(string source, string target, uint flags) {
            OpResult result = new OpResult();
            bool ok = MoveFileExW(source, target, flags);
            int err = Marshal.GetLastWin32Error();
            result.Success = ok;
            result.LastError = ok ? 0 : err;
            result.ErrorName = ErrorName(ok ? 0 : err);
            result.Message = ok ? "" : SystemMessage(err);
            result.Detail = "MoveFileExW flags=0x" + flags.ToString("X");
            return result;
        }

        public static OpResult ReplaceOver(string replaced, string replacement, uint flags) {
            OpResult result = new OpResult();
            bool ok = ReplaceFileW(replaced, replacement, null, flags, IntPtr.Zero, IntPtr.Zero);
            int err = Marshal.GetLastWin32Error();
            result.Success = ok;
            result.LastError = ok ? 0 : err;
            result.ErrorName = ErrorName(ok ? 0 : err);
            result.Message = ok ? "" : SystemMessage(err);
            result.Detail = "ReplaceFileW flags=0x" + flags.ToString("X");
            return result;
        }

        public static RemoteProtocolResult QueryLocality(string path, string driveRoot, bool isDirectory) {
            RemoteProtocolResult result = new RemoteProtocolResult();
            result.Target = path;
            result.InfoClass = FileRemoteProtocolInfo;
            result.DriveType = (int)GetDriveTypeW(driveRoot);
            result.DriveTypeName = DriveTypeName((uint)result.DriveType);
            result.PathIsNetworkPath = PathIsNetworkPathW(path);
            uint flags = isDirectory ? FILE_FLAG_BACKUP_SEMANTICS : 0;
            IntPtr h = CreateFileW(path, 0x80000000, 0x00000007, IntPtr.Zero, OPEN_EXISTING, flags, IntPtr.Zero);
            int openErr = Marshal.GetLastWin32Error();
            if (h == InvalidHandle) {
                result.OpenSuccess = false;
                result.OpenError = openErr;
                result.OpenErrorName = ErrorName(openErr);
                result.Message = SystemMessage(openErr);
                return result;
            }
            result.OpenSuccess = true;
            result.OpenError = 0;
            result.OpenErrorName = ErrorName(0);
            IntPtr buffer = Marshal.AllocHGlobal(512);
            try {
                for (int i = 0; i < 512; i++) { Marshal.WriteByte(buffer, i, 0); }
                bool controlOk = GetFileInformationByHandleEx(h, FileBasicInfo, buffer, 40);
                int controlErr = Marshal.GetLastWin32Error();
                result.ControlCallSuccess = controlOk;
                result.ControlLastError = controlOk ? 0 : controlErr;
                result.ControlErrorName = ErrorName(controlOk ? 0 : controlErr);
                result.ControlDetail = controlOk
                    ? ("FileBasicInfo.FileAttributes=0x" + Marshal.ReadInt32(buffer, 32).ToString("X"))
                    : SystemMessage(controlErr);

                for (int i = 0; i < 512; i++) { Marshal.WriteByte(buffer, i, 0); }
                bool ok = GetFileInformationByHandleEx(h, FileRemoteProtocolInfo, buffer, 116);
                int err = Marshal.GetLastWin32Error();
                result.CallSuccess = ok;
                result.LastError = ok ? 0 : err;
                result.ErrorName = ErrorName(ok ? 0 : err);
                result.Message = ok ? "" : SystemMessage(err);
                if (ok) {
                    result.StructureVersion = (ushort)Marshal.ReadInt16(buffer, 0);
                    result.StructureSize = (ushort)Marshal.ReadInt16(buffer, 2);
                    uint protocol = (uint)Marshal.ReadInt32(buffer, 4);
                    result.Protocol = "0x" + protocol.ToString("X8");
                    result.ProtocolName = ProtocolName(protocol);
                    result.ProtocolMajorVersion = (ushort)Marshal.ReadInt16(buffer, 8);
                    result.ProtocolMinorVersion = (ushort)Marshal.ReadInt16(buffer, 10);
                    result.ProtocolRevision = (ushort)Marshal.ReadInt16(buffer, 12);
                    uint flagBits = (uint)Marshal.ReadInt32(buffer, 16);
                    result.Flags = "0x" + flagBits.ToString("X8");
                    result.FlagNames = FlagNames(flagBits);
                }

                for (int i = 0; i < 512; i++) { Marshal.WriteByte(buffer, i, 0); }
                bool outOfRangeOk = GetFileInformationByHandleEx(h, OutOfRangeInfoClass, buffer, 116);
                int outOfRangeErr = Marshal.GetLastWin32Error();
                result.OutOfRangeClassCallSuccess = outOfRangeOk;
                result.OutOfRangeClassLastError = outOfRangeOk ? 0 : outOfRangeErr;
                result.OutOfRangeClassErrorName = ErrorName(outOfRangeOk ? 0 : outOfRangeErr);
            } finally {
                Marshal.FreeHGlobal(buffer);
                CloseHandle(h);
            }
            return result;
        }
    }
}
