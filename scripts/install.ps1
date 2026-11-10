# CamFacil IA - instalador para Windows 10 e 11.
#   irm https://cfia.ysafe.com.br/instalar.ps1 | iex
# Baixa o programa, instala o ffmpeg se faltar, cria o atalho e abre a dashboard.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$repo = 'emersoncassis/CamFacil-IA'
$dir = Join-Path $env:LOCALAPPDATA 'CamFacilIA'
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$exe = Join-Path $dir 'camfacil.exe'

Write-Host 'Baixando o CamFacil IA...'
Get-Process camfacil -ErrorAction SilentlyContinue | Stop-Process -Force
Invoke-WebRequest "https://github.com/$repo/releases/latest/download/camfacil-windows-x86_64.exe" -OutFile $exe -UseBasicParsing
Unblock-File $exe

$links = Join-Path $env:LOCALAPPDATA 'Microsoft\WinGet\Links\ffmpeg.exe'
if (-not (Get-Command ffmpeg -ErrorAction SilentlyContinue) -and -not (Test-Path $links)) {
  if (Get-Command winget -ErrorAction SilentlyContinue) {
    Write-Host 'Instalando o ffmpeg (mostra o video das cameras)...'
    winget install --id Gyan.FFmpeg -e --silent --accept-source-agreements --accept-package-agreements | Out-Null
  } else {
    Write-Host 'Sem winget: o proprio CamFacil IA vai mostrar como instalar o ffmpeg.'
  }
}

$atalho = Join-Path ([Environment]::GetFolderPath('Desktop')) 'CamFacil IA.lnk'
$s = (New-Object -ComObject WScript.Shell).CreateShortcut($atalho)
$s.TargetPath = $exe
$s.WorkingDirectory = $dir
# Icone do atalho (camera com ponto verde). Se nao baixar, o atalho segue sem ele.
$ico = Join-Path $dir 'camfacil.ico'
try {
  Invoke-WebRequest "https://raw.githubusercontent.com/$repo/main/scripts/icones/camfacil.ico" -OutFile $ico -UseBasicParsing
  $s.IconLocation = "$ico,0"
} catch { Write-Host 'Nao consegui baixar o icone; o atalho fica com o icone padrao.' }
$s.Save()

& $exe --autoiniciar | Out-Null
Write-Host 'Pronto. Atalho criado na area de trabalho e o programa vai abrir junto com o Windows. Abrindo...'
Start-Process $exe
