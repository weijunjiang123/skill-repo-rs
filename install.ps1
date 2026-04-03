$ErrorActionPreference = "Stop"

$repo = "weijunjiang123/skill-repo-rs"
$binary = "skill-repo"
$target = "x86_64-pc-windows-msvc"
$installDir = "$env:USERPROFILE\.local\bin"

Write-Host "检测到平台: $target"

# 获取最新版本
$release = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest"
$tag = $release.tag_name
Write-Host "最新版本: $tag"

# 下载
$url = "https://github.com/$repo/releases/download/$tag/$binary-$target.zip"
Write-Host "下载: $url"

$tmp = New-TemporaryFile | ForEach-Object { Remove-Item $_; New-Item -ItemType Directory -Path $_ }
$zipPath = Join-Path $tmp.FullName "archive.zip"

Invoke-WebRequest -Uri $url -OutFile $zipPath
Expand-Archive -Path $zipPath -DestinationPath $tmp.FullName -Force

# 安装
New-Item -ItemType Directory -Path $installDir -Force | Out-Null
Move-Item (Join-Path $tmp.FullName "$binary.exe") (Join-Path $installDir "$binary.exe") -Force

Remove-Item $tmp.FullName -Recurse -Force

Write-Host ""
Write-Host "已安装到 $installDir\$binary.exe"

# 检查 PATH
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$installDir*") {
    $newPath = "$installDir;$userPath"
    [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
    Write-Host "已将 $installDir 添加到用户 PATH"
    Write-Host "请重启终端使 PATH 生效"
}

Write-Host ""
Write-Host "运行 'skill-repo --help' 开始使用"
