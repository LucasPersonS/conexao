# Conexão: gera o instalador NO SEU PC e publica o release no GitHub, sem GitHub Actions.
# Use quando o Actions der erro de cobrança (minutos esgotados).
#
#   1) Mude a versão em apps/launcher/src-tauri/tauri.conf.json, faça commit, push e a tag
#   2) Rode, no PowerShell, dentro da pasta CENTRAL:   .\lancar-versao-local.ps1
#   3) Na VPS:   bash /opt/conexao/deploy/atualizar.sh
#
# Precisa: Rust, Node e (para publicar sozinho) o GitHub CLI logado: winget install GitHub.cli ; gh auth login
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

$versao = (Get-Content 'apps\launcher\src-tauri\tauri.conf.json' -Raw | ConvertFrom-Json).version
$tag = "v$versao"
$repo = 'LucasPersonS/conexao'
Write-Host "==> Lançando Conexão $tag" -ForegroundColor Green

# Mesmas variáveis do GitHub (Settings > Secrets and variables > Actions > Variables).
$env:CONEXAO_API_URL = 'https://api.conexaosmps.com.br'
if (-not $env:CONEXAO_MANIFEST_PUBKEY) { $env:CONEXAO_MANIFEST_PUBKEY = Read-Host 'Cole a variável CONEXAO_MANIFEST_PUBKEY do GitHub' }
if (-not $env:CONEXAO_MANIFEST_PUBKEY) { throw 'CONEXAO_MANIFEST_PUBKEY é obrigatória (sem ela o launcher não confere os modpacks).' }
if (-not $env:CONEXAO_DISCORD_APP_ID) { $env:CONEXAO_DISCORD_APP_ID = Read-Host 'Cole a variável CONEXAO_DISCORD_APP_ID (Enter para pular)' }

# Chave que assina as atualizações automáticas (a mesma do segredo TAURI_SIGNING_PRIVATE_KEY).
$chave = Join-Path $PSScriptRoot 'apps\launcher\%USERPROFILE%\conexao-updater.key'
if (-not (Test-Path -LiteralPath $chave)) { throw "Não achei a chave do atualizador em $chave" }
$env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content -LiteralPath $chave -Raw).Trim()
if (-not $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD) {
  $senha = Read-Host 'Senha da chave do atualizador (Enter se não tem)' -AsSecureString
  $texto = [Runtime.InteropServices.Marshal]::PtrToStringAuto([Runtime.InteropServices.Marshal]::SecureStringToBSTR($senha))
  if ($texto) { $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $texto }
}

function Rodar([string]$pasta, [string]$comando) {
  Push-Location $pasta
  try {
    cmd /c $comando
    if ($LASTEXITCODE -ne 0) { throw "Falhou: $comando (em $pasta)" }
  } finally { Pop-Location }
}

Write-Host '==> Launcher (instalador + assinatura da atualização)' -ForegroundColor Green
Rodar 'apps\launcher' 'npm ci --no-audit --no-fund'
Rodar 'apps\launcher' 'npx tauri build --config src-tauri/tauri.release.conf.json'

Write-Host '==> Instalador visual (Conexao-Instalador.exe)' -ForegroundColor Green
Rodar 'apps\installer' 'npm ci --no-audit --no-fund'
Rodar 'apps\installer' 'npx tauri build --no-bundle'

$saida = Join-Path $PSScriptRoot "release-$tag"
New-Item -ItemType Directory -Force $saida | Out-Null
$exe = "Conexao_${versao}_x64-setup.exe"
$nsis = Join-Path $PSScriptRoot 'target\release\bundle\nsis'
foreach ($f in @($exe, "$exe.sig")) {
  if (-not (Test-Path (Join-Path $nsis $f))) { throw "O build não gerou $f em $nsis" }
  Copy-Item (Join-Path $nsis $f) $saida -Force
}
Copy-Item (Join-Path $PSScriptRoot 'target\release\conexao-installer.exe') (Join-Path $saida 'Conexao-Instalador.exe') -Force

# latest.json no mesmo formato do tauri-action (o atualizar.sh troca os links pelos da API).
$assinatura = (Get-Content (Join-Path $saida "$exe.sig") -Raw).Trim()
$url = "https://github.com/$repo/releases/download/$tag/$exe"
$plataforma = [ordered]@{ signature = $assinatura; url = $url }
$latest = [ordered]@{
  version   = $versao
  notes     = ''
  pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-dd'T'HH:mm:ss.fff'Z'")
  platforms = [ordered]@{ 'windows-x86_64' = $plataforma; 'windows-x86_64-nsis' = $plataforma }
}
# UTF-8 sem BOM (o Python da VPS recusa BOM).
[IO.File]::WriteAllText((Join-Path $saida 'latest.json'), ($latest | ConvertTo-Json -Depth 5), (New-Object Text.UTF8Encoding $false))

$arquivos = Get-ChildItem $saida | ForEach-Object { $_.FullName }
if (Get-Command gh -ErrorAction SilentlyContinue) {
  Write-Host "==> Publicando o release $tag no GitHub" -ForegroundColor Green
  cmd /c "gh release view $tag --repo $repo >nul 2>nul"
  if ($LASTEXITCODE -eq 0) {
    gh release upload $tag @arquivos --repo $repo --clobber
  } else {
    gh release create $tag @arquivos --repo $repo --title "Conexão $tag" --notes "" --latest
  }
  if ($LASTEXITCODE -ne 0) { throw 'Falha ao publicar no GitHub (rode: gh auth login)' }
  Write-Host "`nPronto! Agora, na VPS:  bash /opt/conexao/deploy/atualizar.sh" -ForegroundColor Green
} else {
  Write-Host "`nGitHub CLI não instalado. Publique na mão:" -ForegroundColor Yellow
  Write-Host "  1. Abra https://github.com/$repo/releases/new?tag=$tag"
  Write-Host "  2. Título: Conexão $tag  e arraste TODOS os arquivos da pasta:"
  Write-Host "     $saida"
  Write-Host "  3. Publish release. Depois, na VPS:  bash /opt/conexao/deploy/atualizar.sh"
  Start-Process explorer.exe $saida
}
