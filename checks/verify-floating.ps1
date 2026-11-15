param([string]$Exe, [string]$OutDir, [string[]]$Runs)
# Shows the floating timer over a plain stand-in window (so no private screen content is
# captured), copies that part of the screen and reports which window is under three points.
# Never moves or clicks the real mouse. Runs are "name=extra arguments", e.g. "bar=--demo bar".
# A run whose name starts with "pulse" (use it with "--demo pulse", which shows the hover
# buttons from 5 s to 7.5 s after the start) is also pictured while they show and after.
Add-Type -AssemblyName System.Windows.Forms,System.Drawing
Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
public class VF {
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
  [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h, uint f);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int w, int hh, uint flags);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
  [DllImport("user32.dll", EntryPoint="GetWindowLongPtrW")] public static extern IntPtr GetWindowLongPtr(IntPtr h, int i);
  [DllImport("user32.dll", EntryPoint="SetWindowLongPtrW")] public static extern IntPtr SetWindowLongPtr(IntPtr h, int i, IntPtr v);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll", EntryPoint="PostMessageW")] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
  delegate bool EnumProc(IntPtr h, IntPtr l);
  public struct POINT { public int X; public int Y; }
  public struct RECT { public int L; public int T; public int R; public int B; }
  // The largest visible top-level window of a process (it also owns tiny helper windows).
  public static IntPtr ByPid(uint pid) { IntPtr found = IntPtr.Zero; int best = 0; EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p); if (p == pid && IsWindowVisible(h)) { RECT r; GetWindowRect(h, out r); int a = (r.R - r.L) * (r.B - r.T); if (a > best) { best = a; found = h; } } return true; }, IntPtr.Zero); return best >= 100 * 50 ? found : IntPtr.Zero; }
  public static IntPtr Hit(int x, int y) { POINT p; p.X = x; p.Y = y; return GetAncestor(WindowFromPoint(p), 2); }
  public static bool IsShown(IntPtr h) { return IsWindowVisible(h); }
  // Pixels (BGRA rows) inside [x0,x1) x [y0,y1) that differ from a colour by more than tol.
  public static int CountNot(byte[] d, int stride, int x0, int y0, int x1, int y1, int r, int g, int b, int tol) {
    int n = 0;
    for (int y = y0; y < y1; y++) for (int x = x0; x < x1; x++) { int i = y * stride + x * 4;
      if (Math.Abs(d[i] - b) > tol || Math.Abs(d[i + 1] - g) > tol || Math.Abs(d[i + 2] - r) > tol) n++; }
    return n;
  }
}
'@
[VF]::SetProcessDPIAware() | Out-Null
if (Get-Process -Name LogonUI -ErrorAction SilentlyContinue) { "LOCKED: nothing checked"; return }
function Pump($ms) { $sw = [Diagnostics.Stopwatch]::StartNew(); while ($sw.ElapsedMilliseconds -lt $ms) { [System.Windows.Forms.Application]::DoEvents(); Start-Sleep -Milliseconds 15 } }
$fg0 = [VF]::GetForegroundWindow()
$back = @(70, 110, 190)
$m = 16

# Copies the timer's part of the screen and says what is under its centre and two corners.
function Snap($tag, $h, $fh, $r) {
  $w = $r.R - $r.L; $hh = $r.B - $r.T
  $c = New-Object VF+POINT; [VF]::GetCursorPos([ref]$c) | Out-Null
  $inside = ($c.X -ge $r.L -and $c.X -lt $r.R -and $c.Y -ge $r.T -and $c.Y -lt $r.B)
  # The window plus a margin, kept on its screen.
  $sb = [System.Windows.Forms.Screen]::FromPoint((New-Object System.Drawing.Point ($r.L + [int]($w / 2)), ($r.T + [int]($hh / 2)))).Bounds
  $x0 = [Math]::Max($r.L - $m, $sb.Left); $y0 = [Math]::Max($r.T - $m, $sb.Top)
  $x1 = [Math]::Min($r.R + $m, $sb.Right); $y1 = [Math]::Min($r.B + $m, $sb.Bottom)
  $bw = $x1 - $x0; $bh = $y1 - $y0
  $bmp = New-Object System.Drawing.Bitmap $bw, $bh, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($x0, $y0, 0, 0, (New-Object System.Drawing.Size $bw, $bh))
  $g.Dispose()
  $file = Join-Path $OutDir "verify-$tag.png"
  $bmp.Save($file)
  $bd = $bmp.LockBits((New-Object System.Drawing.Rectangle 0, 0, $bw, $bh), [System.Drawing.Imaging.ImageLockMode]::ReadOnly, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $bytes = New-Object byte[] ($bd.Stride * $bh)
  [System.Runtime.InteropServices.Marshal]::Copy($bd.Scan0, $bytes, 0, $bytes.Length)
  $bmp.UnlockBits($bd)
  $wx0 = [Math]::Max($r.L, $x0) - $x0; $wy0 = [Math]::Max($r.T, $y0) - $y0; $wx1 = [Math]::Min($r.R, $x1) - $x0; $wy1 = [Math]::Min($r.B, $y1) - $y0
  $drawn = [VF]::CountNot($bytes, $bd.Stride, $wx0, $wy0, $wx1, $wy1, $back[0], $back[1], $back[2], 12)
  # Anything but the stand-in's colour around the window means something else was pictured.
  $frame = [VF]::CountNot($bytes, $bd.Stride, 0, 0, $bw, $bh, $back[0], $back[1], $back[2], 12) - $drawn
  $bmp.Dispose()
  # Without the stand-in behind it the picture would show whatever is on the person's screen.
  if ($frame -gt 0) { [System.IO.File]::Delete($file); "${tag}: stand-in window not behind the timer, picture not kept" }
  $who = { param($x) if ($x -eq $h) { "timer" } elseif ($x -eq $fh) { "window behind" } else { "other" } }
  "{0}: window {1},{2} {3}x{4}; pointer inside={5}; painted {6:P1} of the window; centre -> {7}; top-left corner -> {8}; bottom-right corner -> {9}; {10}" -f `
    $tag, $r.L, $r.T, $w, $hh, $inside, ($drawn / ($w * $hh)), (& $who ([VF]::Hit($r.L + [int]($w / 2), $r.T + [int]($hh / 2)))), (& $who ([VF]::Hit($r.L + 6, $r.T + 6))), (& $who ([VF]::Hit($r.R - 6, $r.B - 6))), (Styles $h)
}

# The extended styles that decide task bar, Alt+Tab and focus.
function Styles($h) {
  $ex = [int64][VF]::GetWindowLongPtr($h, -20)
  "shown={0} tool window={1} task-bar window={2} no-activate={3} top-most={4}" -f [VF]::IsShown($h), (($ex -band 0x80) -ne 0), (($ex -band 0x40000) -ne 0), (($ex -band 0x8000000) -ne 0), (($ex -band 0x8) -ne 0)
}

foreach ($run in $Runs) {
  $name, $rest = $run -split '=', 2
  # A run whose name starts with "float" opens as the editor (use it with "--demo float", which
  # turns the editor into the floating timer after 90 frames, as its button does).
  $fromEditor = $name -like 'float*'
  $argList = @($rest -split ' ' | Where-Object { $_ })
  if (-not $fromEditor) { $argList = @("--overlay") + $argList }
  $clock = [Diagnostics.Stopwatch]::StartNew()
  $p = Start-Process -FilePath $Exe -ArgumentList $argList -PassThru
  $h = [IntPtr]::Zero
  for ($i = 0; $i -lt 80 -and $h -eq [IntPtr]::Zero; $i++) { Start-Sleep -Milliseconds 100; $h = [VF]::ByPid([uint32]$p.Id) }
  if ($h -eq [IntPtr]::Zero) { "${name}: no window"; $p.Kill(); continue }
  if ($fromEditor) { Start-Sleep -Milliseconds 4500 } else { Start-Sleep -Milliseconds 1300 }
  $r = New-Object VF+RECT; [VF]::GetWindowRect($h, [ref]$r) | Out-Null
  $w = $r.R - $r.L; $hh = $r.B - $r.T
  $form = New-Object System.Windows.Forms.Form
  $form.FormBorderStyle = 'None'; $form.StartPosition = 'Manual'; $form.ShowInTaskbar = $false
  $form.BackColor = [System.Drawing.Color]::FromArgb($back[0], $back[1], $back[2])
  # Top-most like the timer (a background process can't raise an ordinary window above the
  # window in front), then slipped in right below the timer.
  $form.TopMost = $true
  $fh = $form.Handle
  $ex = [int64][VF]::GetWindowLongPtr($fh, -20)
  [VF]::SetWindowLongPtr($fh, -20, [IntPtr]($ex -bor 0x08000000 -bor 0x80)) | Out-Null
  $pad = 40
  # A process started hidden has its first ShowWindow call overridden, so ask twice.
  [VF]::ShowWindow($fh, 0) | Out-Null
  [VF]::ShowWindow($fh, 4) | Out-Null
  [VF]::SetWindowPos($fh, $h, $r.L - $pad, $r.T - $pad, $w + 2 * $pad, $hh + 2 * $pad, 0x10) | Out-Null
  Pump 1600
  Snap $name $h $fh $r
  if ($name -like 'pulse*') {
    # While the buttons show the whole window should take the mouse; afterwards only the
    # painted parts again, looking as it did before.
    Pump ([Math]::Max(0, 6700 - $clock.ElapsedMilliseconds))
    Snap "$name-during" $h $fh $r
    Pump ([Math]::Max(0, 10600 - $clock.ElapsedMilliseconds))
    Snap "$name-after" $h $fh $r
  }
  if ($name -like 'floatback*') {
    # With "--demo float,back" the timer goes back to the editor after 600 frames, as its gear
    # button does: the editor's size and a task-bar button should be back. Not pictured.
    $form.Hide()
    Pump ([Math]::Max(0, 15000 - $clock.ElapsedMilliseconds))
    $r2 = New-Object VF+RECT; [VF]::GetWindowRect($h, [ref]$r2) | Out-Null
    "{0}-back: window {1},{2} {3}x{4}; {5}" -f $name, $r2.L, $r2.T, ($r2.R - $r2.L), ($r2.B - $r2.T), (Styles $h)
  }
  $p.Kill()
  $form.Close(); $form.Dispose()
  Start-Sleep -Milliseconds 350
}
"foreground window unchanged: " + ([VF]::GetForegroundWindow() -eq $fg0)
