# Walks a built Event Studio through its whole setup path inside one new folder: make a setup
# file, install from it, start the installed program, remove it again (in Korean and, without
# settings, in Japanese). Everything stays in
# -Work (the program's --sandbox option keeps the Start menu, the desktop and the app-list entry
# inside it too), so nothing of the person's is touched. Never moves the mouse.
#
# Usage: pwsh -File verify-setup.ps1 -Exe <built exe> -Work <new folder in %TEMP%> [-Settings <settings.json to carry>] [-Shots]
param(
  [Parameter(Mandatory)][string]$Exe,
  [Parameter(Mandatory)][string]$Work,
  [string]$Settings = '',
  [switch]$Shots
)
$ErrorActionPreference = 'Stop'
if (Test-Path -LiteralPath $Work) { throw "Work folder already exists: $Work" }
$src = Join-Path $Work 'src'
New-Item -ItemType Directory -Path $src | Out-Null
Copy-Item -LiteralPath $Exe -Destination (Join-Path $src 'EventStudio.exe')
$carry = $false
if ($Settings) {
  # A copy with the global hotkeys off, so a check never takes keys away from the person.
  $s = Get-Content -LiteralPath $Settings -Raw -Encoding UTF8 | ConvertFrom-Json
  $s.hotkeys = $false
  $s.overlay_start = $false
  [System.IO.File]::WriteAllText((Join-Path $src 'settings.json'), ($s | ConvertTo-Json -Depth 6), (New-Object System.Text.UTF8Encoding($false)))
  $from = Join-Path (Split-Path -Parent $Settings) 'assets'
  if (Test-Path -LiteralPath $from) { Copy-Item -LiteralPath $from -Destination (Join-Path $src 'assets') -Recurse }
  $carry = $true
}
$results = New-Object System.Collections.Generic.List[string]
function Check([string]$name, [bool]$ok, [string]$detail = '') {
  $results.Add(('{0}  {1}{2}' -f $(if ($ok) { 'PASS' } else { 'FAIL' }), $name, $(if ($detail) { "  ($detail)" } else { '' })))
}
function Quote([string]$s) { '"' + $s + '"' }
# Starts a program and waits for it to end; returns its exit code, or 'timeout'.
function Run([string]$file, [string]$arguments, [int]$seconds = 60) {
  $p = Start-Process -FilePath $file -ArgumentList $arguments -PassThru
  if (-not $p.WaitForExit($seconds * 1000)) { try { $p.Kill() } catch {}; return 'timeout' }
  return $p.ExitCode
}
function WaitFor([scriptblock]$done, [int]$seconds = 30) {
  $until = (Get-Date).AddSeconds($seconds)
  while ((Get-Date) -lt $until) { if (& $done) { return $true }; Start-Sleep -Milliseconds 250 }
  return $false
}
function Hash([string]$file) { (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash }
function PictureOk([string]$file) {
  if (-not (Test-Path -LiteralPath $file)) { return $false }
  Add-Type -AssemblyName System.Drawing
  $bmp = [System.Drawing.Bitmap]::FromFile($file)
  try {
    $seen = New-Object System.Collections.Generic.HashSet[int]
    for ($y = 5; $y -lt $bmp.Height; $y += [Math]::Max(1, [int]($bmp.Height / 40))) {
      for ($x = 5; $x -lt $bmp.Width; $x += [Math]::Max(1, [int]($bmp.Width / 40))) { [void]$seen.Add($bmp.GetPixel($x, $y).ToArgb()) }
    }
    return $seen.Count -ge 4
  } finally { $bmp.Dispose() }
}

$app = Join-Path $src 'EventStudio.exe'
$setup = Join-Path $Work 'EventStudio_Setup.exe'
$box = Join-Path $Work 'box'
$dir = Join-Path $box 'Programs\Event Studio'
$installed = Join-Path $dir 'EventStudio.exe'
$menu = Join-Path $box 'StartMenu\Event Studio'
$desktop = Join-Path $box 'Desktop\Event Studio.lnk'
$entry = Join-Path $box 'app-list-entry.json'
$shotDir = Join-Path $Work 'shots'
# Korean wording is asked for, so the checks below can look for it on a computer in any language.
$sandbox = '--lang ko --sandbox ' + (Quote $box)
$guides = @('사용안내.txt', 'Guide.txt', '使用说明.txt', '使い方.txt')

# 1. The program packs itself.
$code = Run $app ('--make-setup ' + (Quote $setup) + ' --log ' + (Quote (Join-Path $Work 'make.log')))
Check 'setup file made' ($code -eq 0 -and (Test-Path -LiteralPath $setup)) "exit $code"
$grown = (Get-Item -LiteralPath $setup).Length - (Get-Item -LiteralPath $app).Length
Check 'setup file is the program plus its pack' ($grown -gt 40 -and $grown -lt 60MB) "$grown bytes added"
$made = Get-Content -LiteralPath (Join-Path $Work 'make.log') -Encoding UTF8
Check 'settings carried as asked' (($made -contains 'settings.json') -eq $carry) ($made -join ', ')

# 2. Pictures of the windows (optional; the windows stay beyond the edge of the screen).
if ($Shots) {
  foreach ($shot in @(@('install', '--demo hidden'), @('install-done', '--demo hidden,done'), @('install-existing', '--demo hidden,existing,error'))) {
    $png = Join-Path $shotDir ($shot[0] + '.png')
    $code = Run $setup ("$sandbox --screenshot " + (Quote $png) + ' ' + $shot[1])
    Check ("picture of setup window: " + $shot[0]) ($code -eq 0 -and (PictureOk $png)) "exit $code"
  }
  Check 'pictures did not install anything' (-not (Test-Path -LiteralPath $box))
}

# 3. Install without a window.
$log = Join-Path $Work 'install.log'
$code = Run $setup ("--silent $sandbox --log " + (Quote $log))
Check 'install ends well' ($code -eq 0) ("exit $code; " + ((Get-Content -LiteralPath $log -Encoding UTF8) -join ' | '))
Check 'installed program is the program without the pack' ((Test-Path -LiteralPath $installed) -and (Hash $installed) -eq (Hash $app))
$missing = @($guides + 'install-info.txt' | Where-Object { -not (Test-Path -LiteralPath (Join-Path $dir $_)) })
Check 'guides in four languages and install mark written' ($missing.Count -eq 0) ($missing -join ', ')
if ($carry) {
  Check 'settings installed unchanged' ((Hash (Join-Path $dir 'settings.json')) -eq (Hash (Join-Path $src 'settings.json')))
  $assets = @(Get-ChildItem -LiteralPath (Join-Path $src 'assets') -File -ErrorAction SilentlyContinue)
  $named = Get-Content -LiteralPath (Join-Path $dir 'settings.json') -Raw -Encoding UTF8 | ConvertFrom-Json
  foreach ($name in @($named.background, $named.logo, $named.logo2, $named.sound_file) | Where-Object { $_ }) {
    Check "image or sound the settings name: $name" (Test-Path -LiteralPath (Join-Path $dir $name))
  }
} else {
  $first = Get-Content -LiteralPath (Join-Path $dir 'settings.json') -Raw -Encoding UTF8 | ConvertFrom-Json
  Check 'first settings say which language to open in' ($first.language -eq 'ko') $first.language
}
# Shortcuts, read back by Windows' own script host.
$wsh = New-Object -ComObject WScript.Shell
$links = @(
  @((Join-Path $menu 'Event Studio.lnk'), $installed, ''),
  @((Join-Path $menu '사용안내.lnk'), (Join-Path $dir '사용안내.txt'), ''),
  @((Join-Path $menu 'Event Studio 제거.lnk'), $installed, '--uninstall'),
  @($desktop, $installed, '')
)
foreach ($l in $links) {
  if (Test-Path -LiteralPath $l[0]) {
    $read = $wsh.CreateShortcut($l[0])
    $ok = ($read.TargetPath -eq $l[1]) -and ($read.WorkingDirectory -eq $dir) -and ($l[2] -eq '' -and $read.Arguments -eq '' -or $l[2] -ne '' -and $read.Arguments.StartsWith($l[2]))
    Check ("shortcut " + (Split-Path -Leaf $l[0])) $ok ("-> " + $read.TargetPath + ' ' + $read.Arguments)
  } else { Check ("shortcut " + (Split-Path -Leaf $l[0])) $false 'missing' }
}
$e = Get-Content -LiteralPath $entry -Raw -Encoding UTF8 | ConvertFrom-Json
Check 'app-list entry' ($e.DisplayName -eq 'Event Studio' -and $e.InstallLocation -eq $dir -and $e.UninstallString.StartsWith((Quote $installed) + ' --uninstall') -and $e.NoModify -eq 1 -and $e.EstimatedSize -gt 1000) ($e.DisplayVersion + ', ' + $e.EstimatedSize + ' KB')

# 4. The installed program starts and draws.
$png = Join-Path $shotDir 'installed-program.png'
$code = Run $installed ('--demo hidden --screenshot ' + (Quote $png))
Check 'installed program starts and draws' ($code -eq 0 -and (PictureOk $png)) "exit $code"
Check 'a look at the program changed no settings' (-not $carry -or (Hash (Join-Path $dir 'settings.json')) -eq (Hash (Join-Path $src 'settings.json')))

# 5. Installing again keeps what is saved.
Add-Content -LiteralPath (Join-Path $dir 'settings.json') -Value ' ' -Encoding UTF8
$kept = Hash (Join-Path $dir 'settings.json')
$code = Run $setup ("--silent $sandbox --log " + (Quote $log))
$said = (Get-Content -LiteralPath $log -Encoding UTF8) -join ' | '
Check 'installing again keeps saved settings' ($code -eq 0 -and (Hash (Join-Path $dir 'settings.json')) -eq $kept -and ($said -match 'KeptExisting') -eq $carry) $said

# 5b. A running program is not replaced: the setup says so, and works once the program has ended.
# (The program is started for a check picture, which keeps it open for about two seconds
# without taking the keyboard.)
function StartBusy([string]$name) {
  Start-Process -FilePath $installed -ArgumentList ('--screenshot ' + (Quote (Join-Path $shotDir $name)) + ' --demo hidden,packer') -PassThru
}
$busy = StartBusy 'busy-install.png'
Start-Sleep -Milliseconds 600
$code = Run $setup ("--silent $sandbox --log " + (Quote $log))
$wasRunning = -not $busy.HasExited
$said = (Get-Content -LiteralPath $log -Encoding UTF8) -join ' | '
[void]$busy.WaitForExit(30000)
Check 'a running program is not replaced' ($wasRunning -and $code -eq 1 -and $said -match '실행 중') ("running: $wasRunning; exit $code; $said")
$code = Run $setup ("--silent $sandbox --log " + (Quote $log))
Check 'install works again once the program has ended' ($code -eq 0) "exit $code"

# 6. Removal, asked of the installed program as Windows' app list does.
if ($Shots) {
  $png = Join-Path $shotDir 'remove.png'
  $code = Run $installed ("--uninstall $sandbox --demo hidden --screenshot " + (Quote $png))
  $seen = WaitFor { Test-Path -LiteralPath $png }
  Start-Sleep -Milliseconds 800
  Check 'picture of removal window' ($code -eq 0 -and $seen -and (PictureOk $png)) "exit $code"
  Check 'picture did not remove anything' (Test-Path -LiteralPath $installed)
}
$log = Join-Path $Work 'remove.log'
$code = Run $installed ("--uninstall --silent $sandbox --log " + (Quote $log))
$gone = WaitFor { (Test-Path -LiteralPath $log) -and -not (Test-Path -LiteralPath $installed) }
$said = if (Test-Path -LiteralPath $log) { (Get-Content -LiteralPath $log -Encoding UTF8) -join ' | ' } else { 'no log' }
Check 'removal ends well' ($code -eq 0 -and $gone -and $said.StartsWith('ok')) $said
$left = @($guides + 'install-info.txt' + 'EventStudio.exe' | Where-Object { Test-Path -LiteralPath (Join-Path $dir $_) })
Check 'program, guides, mark, shortcuts and entry are gone' ($left.Count -eq 0 -and -not ((Test-Path -LiteralPath $menu) -or (Test-Path -LiteralPath $desktop) -or (Test-Path -LiteralPath $entry))) ($left -join ', ')
Check 'settings stay unless asked' (Test-Path -LiteralPath (Join-Path $dir 'settings.json'))

# 7. Installed once more and removed with the settings while the program is still open: the
# removal waits for it to end, then the folder itself goes.
$code = Run $setup ("--silent $sandbox --log " + (Quote (Join-Path $Work 'install.log')))
Remove-Item -LiteralPath $log
$busy = StartBusy 'busy-remove.png'
Start-Sleep -Milliseconds 600
$code2 = Run $installed ("--uninstall --silent --remove-data $sandbox --log " + (Quote $log))
$wasRunning = -not $busy.HasExited
$gone = WaitFor { (Test-Path -LiteralPath $log) -and -not (Test-Path -LiteralPath $dir) }
$said = if (Test-Path -LiteralPath $log) { (Get-Content -LiteralPath $log -Encoding UTF8) -join ' | ' } else { 'no log' }
Check 'removal waits for the program to end' ($wasRunning -and $said.StartsWith('ok')) ("running: $wasRunning; $said")
Check 'removal with settings leaves no folder' ($code -eq 0 -and $code2 -eq 0 -and $gone) ((Get-ChildItem -LiteralPath $box -Recurse -File -ErrorAction SilentlyContinue | ForEach-Object { $_.FullName.Substring($box.Length) }) -join ', ')

# 7b. A setup without settings, run in Japanese: Japanese shortcut names, and the program is told to open in Japanese.
$plain = Join-Path $Work 'Plain_Setup.exe'
$boxJa = Join-Path $Work 'box-ja'
$dirJa = Join-Path $boxJa 'Programs\Event Studio'
$menuJa = Join-Path $boxJa 'StartMenu\Event Studio'
$code = Run $app ('--make-setup ' + (Quote $plain) + ' --no-settings')
$logJa = Join-Path $Work 'install-ja.log'
$code2 = Run $plain ('--silent --lang ja --sandbox ' + (Quote $boxJa) + ' --log ' + (Quote $logJa))
Check 'Japanese install ends well' ($code -eq 0 -and $code2 -eq 0) ((Get-Content -LiteralPath $logJa -Encoding UTF8) -join ' | ')
$firstJa = Get-Content -LiteralPath (Join-Path $dirJa 'settings.json') -Raw -Encoding UTF8 | ConvertFrom-Json
Check 'the program is told to open in Japanese' ($firstJa.language -eq 'ja' -and $firstJa.event -eq '新しい可能性をひらく時間') ($firstJa.language + ' / ' + $firstJa.event)
$guideJa = Join-Path $menuJa '使い方.lnk'
$removeJa = Join-Path $menuJa 'Event Studio のアンインストール.lnk'
$okJa = (Test-Path -LiteralPath $guideJa) -and (Test-Path -LiteralPath $removeJa) -and ($wsh.CreateShortcut($guideJa).TargetPath -eq (Join-Path $dirJa '使い方.txt')) -and -not (Test-Path -LiteralPath (Join-Path $menuJa '사용안내.lnk'))
Check 'Japanese shortcut names lead to the Japanese guide' $okJa ((Get-ChildItem -LiteralPath $menuJa -ErrorAction SilentlyContinue | ForEach-Object { $_.Name }) -join ', ')
$pngJa = Join-Path $shotDir 'installed-japanese.png'
$code = Run (Join-Path $dirJa 'EventStudio.exe') ('--demo hidden --screenshot ' + (Quote $pngJa))
Check 'the Japanese install starts and draws' ($code -eq 0 -and (PictureOk $pngJa)) "exit $code"
$logJa = Join-Path $Work 'remove-ja.log'
$code = Run (Join-Path $dirJa 'EventStudio.exe') ('--uninstall --silent --remove-data --sandbox ' + (Quote $boxJa) + ' --log ' + (Quote $logJa))
$gone = WaitFor { (Test-Path -LiteralPath $logJa) -and -not (Test-Path -LiteralPath $dirJa) }
$said = if (Test-Path -LiteralPath $logJa) { (Get-Content -LiteralPath $logJa -Encoding UTF8) -join ' | ' } else { 'no log' }
Check 'the Japanese install is removed without a trace' ($code -eq 0 -and $gone -and -not (Test-Path -LiteralPath $menuJa)) $said

# 8. The copies removals worked from are cleared by the next start of the program.
$retired = Join-Path $env:TEMP 'EventStudioUninstall'
Start-Sleep -Milliseconds 800
$before = @(Get-ChildItem -LiteralPath $retired -File -ErrorAction SilentlyContinue).Count
$code = Run $app ('--demo hidden --screenshot ' + (Quote (Join-Path $shotDir 'after.png')))
$after = @(Get-ChildItem -LiteralPath $retired -File -ErrorAction SilentlyContinue).Count
Check 'working copies of removals are cleared at the next start' ($before -ge 1 -and $after -eq 0) "$before before, $after after"

$results
if ($results | Where-Object { $_.StartsWith('FAIL') }) { exit 1 }
