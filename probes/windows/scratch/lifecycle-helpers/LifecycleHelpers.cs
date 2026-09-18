using System;
using System.Diagnostics;
using System.Threading;
using System.Windows.Forms;

namespace AgentDesktop.LifecycleHelpers
{
    internal static class Program
    {
        private static int Main(string[] args)
        {
            string mode = "wait";
            string childPath = null;
            string childArgs = "";
            int sleepMs = 60000;
            for (int i = 0; i < args.Length; i++)
            {
                string a = args[i];
                if (a == "--mode" && i + 1 < args.Length)
                {
                    mode = args[++i];
                }
                else if (a == "--child" && i + 1 < args.Length)
                {
                    childPath = args[++i];
                }
                else if (a == "--child-args" && i + 1 < args.Length)
                {
                    childArgs = args[++i];
                }
                else if (a == "--sleep-ms" && i + 1 < args.Length)
                {
                    int.TryParse(args[++i], out sleepMs);
                }
            }

            if (mode == "clean-exit")
            {
                return 0;
            }
            if (mode == "crash-exit")
            {
                return unchecked((int)0xC0000005);
            }
            if (mode == "child-spawner")
            {
                if (string.IsNullOrEmpty(childPath))
                {
                    return 2;
                }
                ProcessStartInfo psi = new ProcessStartInfo();
                psi.FileName = childPath;
                psi.Arguments = childArgs;
                psi.UseShellExecute = false;
                Process child = Process.Start(psi);
                if (child == null)
                {
                    return 3;
                }
                Thread.Sleep(sleepMs);
                try
                {
                    if (!child.HasExited)
                    {
                        child.Kill();
                    }
                }
                catch
                {
                }
                return 0;
            }
            if (mode == "stalled")
            {
                // STA thread creates a Form and never pumps — SendMessageTimeout(SMTO_ABORTIFHUNG)
                // and IsHungAppWindow observe the hang; Main blocks so the process stays alive.
                Thread stalledThread = new Thread(new ThreadStart(RunStalledForm));
                stalledThread.SetApartmentState(ApartmentState.STA);
                stalledThread.IsBackground = true;
                stalledThread.Start();
                Thread.Sleep(sleepMs);
                return 0;
            }
            if (mode == "window")
            {
                Application.EnableVisualStyles();
                Application.SetCompatibleTextRenderingDefault(false);
                Form form = new Form();
                form.Text = "a21-lifecycle-window";
                form.StartPosition = FormStartPosition.Manual;
                form.Bounds = new System.Drawing.Rectangle(140, 140, 420, 280);
                form.ShowInTaskbar = false;
                Application.Run(form);
                return 0;
            }
            if (mode == "modal")
            {
                // A real Win32 MessageBox: owned by a visible top-level Form, blocks the
                // calling thread in its own modal loop until dismissed (WM_CLOSE posted to
                // the dialog by an external caller unblocks it). Windows applies
                // WS_EX_DLGMODALFRAME and an owner relationship to a MessageBox by
                // construction, which is exactly the property area 23's modal-classification
                // leg needs to observe from outside the process.
                Application.EnableVisualStyles();
                Form owner = new Form();
                owner.Text = "a23-modal-owner";
                owner.StartPosition = FormStartPosition.Manual;
                owner.Bounds = new System.Drawing.Rectangle(160, 160, 320, 220);
                owner.ShowInTaskbar = false;
                owner.Show();
                MessageBox.Show(owner, "a23-modal-body", "a23-modal-title", MessageBoxButtons.OKCancel);
                owner.Close();
                return 0;
            }

            Thread.Sleep(sleepMs);
            return 0;
        }

        private static void RunStalledForm()
        {
            Form form = new Form();
            form.Text = "a21-stalled";
            form.ShowInTaskbar = false;
            form.StartPosition = FormStartPosition.Manual;
            form.Bounds = new System.Drawing.Rectangle(80, 80, 320, 200);
            form.Show();
            while (true)
            {
                Thread.Sleep(50);
            }
        }
    }
}
