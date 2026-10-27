param([Parameter(Mandatory=$true)][string]$Binary)
$ErrorActionPreference = 'Stop'
$root = Join-Path $PSScriptRoot '../.work/windows-pipe'
New-Item -ItemType Directory -Force $root | Out-Null
$root = (Resolve-Path $root).Path
$info = [Diagnostics.ProcessStartInfo]::new((Resolve-Path $Binary).Path)
$info.UseShellExecute = $false
$info.RedirectStandardInput = $true
$info.RedirectStandardOutput = $true
$info.RedirectStandardError = $true
$info.CreateNoWindow = $true
$p = [Diagnostics.Process]::Start($info)
$p.StandardInput.NewLine = "`n"
function Read-Frame {
  $task = $p.StandardOutput.ReadLineAsync()
  if (-not $task.Wait(5000)) { throw 'Protocol timeout' }
  if (-not $task.Result) { throw 'Unexpected EOF' }
  return $task.Result | ConvertFrom-Json
}
function Request-Frame($method, $params) {
  $rid = 'h:' + [guid]::NewGuid().ToString()
  $p.StandardInput.WriteLine((@{type='request';id=$rid;method=$method;params=$params} | ConvertTo-Json -Depth 30 -Compress))
  $p.StandardInput.Flush()
  $response = Read-Frame
  if ($response.id -ne $rid -or $response.error) { throw 'Unexpected/error response' }
  return $response.result
}
try {
  $hello = Read-Frame
  if ($hello.id -ne 'com.tezbar.native.qrcode-generator' -or $hello.protocol -ne 1) { throw 'Bad hello' }
  $ready = Request-Frame 'initialize' @{protocol=1;extensionId=$hello.id;platform='Windows';packageDir=$root;dataDir=(Join-Path $root 'data');tempDir=(Join-Path $root 'temp');grantedCapabilities=@()}
  if (-not $ready.ready) { throw 'Not ready' }
  $form = Request-Frame 'session.open' @{sessionId='form';commandName='index';preferences=@{};argumentValues=@{}}
  if ($form.root.type -ne 'Form') { throw 'Missing Form' }
  $values = @{url='Windows native QR smoke';format='png';color='custom';customColor='#000000';shorten=$false;utmEnabled=$false;__tezbarDraftRevision=1}
  $detail = Request-Frame 'session.call' @{sessionId='form';operation='extension:invoke-action';payload=@{actionId='generate-show';formValues=$values}}
  if (-not $detail.root.props.markdown.StartsWith('![qrcode](data:image/png;base64,')) { throw 'Missing generated PNG preview' }
  $null = Request-Frame 'session.dispose' @{sessionId='form'}
  $null = Request-Frame 'shutdown' @{}
  $p.StandardInput.Close()
  if (-not $p.WaitForExit(6000) -or $p.ExitCode -ne 0) { throw 'Process did not exit cleanly' }
  Write-Host 'Windows native MSVC executable JSONL smoke passed; no desktop services invoked.'
} finally {
  if (-not $p.HasExited) { $p.Kill($true) }
  $p.Dispose()
}
