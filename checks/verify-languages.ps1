# Takes pictures of every screen in every language, with the windows kept beyond the edge of the
# screen (--demo hidden), so nothing shows up in front of the person and the mouse is never used.
# The program only draws: --screenshot never saves settings, installs or removes anything.
#
# Usage: pwsh -File verify-languages.ps1 -Exe <built exe> -OutDir <new folder in %TEMP%> [-Languages ko,en,zh,ja]
param(
  [Parameter(Mandatory)][string]$Exe,
  [Parameter(Mandatory)][string]$OutDir,
  [string[]]$Languages = @('ko', 'en', 'zh', 'ja')
)
$ErrorActionPreference = 'Stop'
if (Test-Path -LiteralPath $OutDir) { throw "Folder already exists: $OutDir" }
New-Item -ItemType Directory -Path $OutDir | Out-Null
function Quote([string]$s) { '"' + $s + '"' }
function Run([string]$file, [string]$arguments, [int]$seconds = 60) {
  $p = Start-Process -FilePath $file -ArgumentList $arguments -PassThru
  if (-not $p.WaitForExit($seconds * 1000)) { try { $p.Kill() } catch {}; return 'timeout' }
  return $p.ExitCode
}
# A setup file to show the setup and removal windows with (it carries whatever settings sit beside the program).
$setup = Join-Path $OutDir 'Setup.exe'
$code = Run $Exe ('--make-setup ' + (Quote $setup))
if ($code -ne 0) { throw "could not make a setup file (exit $code)" }
$box = Join-Path $OutDir 'box'
$installed = Join-Path $box 'Programs\Event Studio'
$results = New-Object System.Collections.Generic.List[string]
foreach ($lang in $Languages) {
  $shots = @(
    @('editor', $Exe, '--demo hidden'),
    @('editor-narrow', $Exe, '--demo hidden,narrow'),
    @('tab-time', $Exe, '--demo hidden,timetab'),
    @('tab-floating', $Exe, '--demo hidden,floattab'),
    @('bar-warning', $Exe, '--demo hidden,bar,warning,hover'),
    @('number-over', $Exe, '--demo hidden,number,over'),
    @('make-setup', $Exe, '--demo hidden,packer'),
    @('floating', $Exe, '--overlay --demo hidden,warning,hover'),
    @('floating-number', $Exe, '--overlay --demo hidden,number,over'),
    @('setup', $setup, ('--sandbox ' + (Quote $box) + ' --demo hidden')),
    @('setup-existing', $setup, ('--sandbox ' + (Quote $box) + ' --demo hidden,existing,error')),
    @('setup-done', $setup, ('--sandbox ' + (Quote $box) + ' --demo hidden,done')),
    @('remove', $setup, ('--uninstall-from ' + (Quote $installed) + ' --sandbox ' + (Quote $box) + ' --demo hidden')),
    @('remove-done', $setup, ('--uninstall-from ' + (Quote $installed) + ' --sandbox ' + (Quote $box) + ' --demo hidden,removed'))
  )
  foreach ($s in $shots) {
    $png = Join-Path $OutDir ($lang + '-' + $s[0] + '.png')
    $code = Run $s[1] ($s[2] + ' --lang ' + $lang + ' --screenshot ' + (Quote $png))
    $ok = ($code -eq 0) -and (Test-Path -LiteralPath $png) -and ((Get-Item -LiteralPath $png).Length -gt 3000)
    $results.Add(('{0}  {1}-{2}{3}' -f $(if ($ok) { 'PASS' } else { 'FAIL' }), $lang, $s[0], $(if ($ok) { '' } else { "  (exit $code)" })))
  }
}
# Switching while the program runs, the way the language box does it (texts, fonts and title change mid-run).
foreach ($pair in @(@('ko', 'en'), @('ko', 'zh'), @('ko', 'ja'), @('ja', 'ko'), @('zh', 'en'))) {
  $png = Join-Path $OutDir ('switch-' + $pair[0] + '-to-' + $pair[1] + '.png')
  $code = Run $Exe ('--demo hidden,floattab,to-' + $pair[1] + ' --lang ' + $pair[0] + ' --screenshot ' + (Quote $png))
  $ok = ($code -eq 0) -and (Test-Path -LiteralPath $png) -and ((Get-Item -LiteralPath $png).Length -gt 3000)
  $results.Add(('{0}  switch {1} -> {2}{3}' -f $(if ($ok) { 'PASS' } else { 'FAIL' }), $pair[0], $pair[1], $(if ($ok) { '' } else { "  (exit $code)" })))
}
$results
"pictures in $OutDir; nothing installed: " + (-not (Test-Path -LiteralPath $box))
if ($results | Where-Object { $_.StartsWith('FAIL') }) { exit 1 }
