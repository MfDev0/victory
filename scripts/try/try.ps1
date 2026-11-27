# VIctory Try: builds this fork's VIctory on your PC and runs your game with
# it. Started by VIctoryTry.exe (see scripts/try/README.md).
#
# Nothing is sent anywhere: no sign-in, no reports, no accounts. It only
# downloads (anonymously) the fork's source, upstream VIctory's new commits
# and, the first time, Git, Rust and Microsoft's C++ build tools.
#
# Upstream's new fixes are added by themselves: the branch's
# scripts/try/upstream.txt names the last upstream commit the fork already
# has; each later one is applied on top, oldest first. Ones already in the
# fork (empty) or conflicting are skipped, and if the build fails with them
# the fork is built alone.
#
# ASCII only: Windows PowerShell 5.1 reads a script without a BOM as ANSI.

$ErrorActionPreference = 'Continue'
$ProgressPreference = 'SilentlyContinue'
$TryVersion = '6.22'
$ForkUrl = 'https://github.com/MfDev0/victory.git'
$Branch = 'arena/59fabb00-victory'
# Other places the fork may live, tried when the fork itself cannot be
# reached. 6.17: first CATez, the fork's own copy outside the fork network
# (a repository of its own, not a GitHub fork: it stays when VIctory's
# repository or the forks it came from go), then a backup copy.
$MirrorUrls = @('https://github.com/MfDev0/CATez.git', 'https://github.com/MfDev0/CAtez-backup.git')
$UpstreamUrl = 'https://github.com/Marvisak/VIctory.git'
# 5.9: the 8 GB NVIDIA engine, soldsawn's build of fxceless/victory-fork (the
# community's main line): measured on an RTX 4060 Laptop 8 GB at 8-10 fps
# with clean textures (its README). Its own source and build folders; this
# fork's source folder stays, so VIctory Try still updates itself from it.
$Engine8Url = 'https://github.com/soldsawn/victory-nvidia-8gb.git'
$Engine8Branch = 'main'
# 6.10: this fork's engine is the default again (N still picks this one).
# 6.9: the AvatarManz engine, an option. PortWatch (2026-11-26): Belobug
# #1 (about 22 fps on an RX 9070 XT, picture close to the console) and
# AvatarManz #2 (16 fps on an RX 7900 XTX, 45 ms a frame on Series S) are the
# same engine: Belobug's a-publier branch is avatarmanz/VIctory-7900XTX
# c9c932c (renderer ports 1-7, list calls on eight threads, textures on their
# own copy queue, every shader translated ahead of time) plus Belobug's flicker
# fix (title textures uploaded in the submission's prelude), a texture cap that
# follows the card's free memory (what keeps a 6 GB card from overflowing) and
# PC FSR 4.1.2. VICTORY_TRY_AVM=avatarmanz builds avatarmanz main instead.
# Built with their own command (diag profile, diagnostics: the build all their
# measurements are of) and run with their scripts/play.ps1 settings.
$EngineAvmUrl = 'https://github.com/Belobug/VIctory-Belobug-AMD.git'
$EngineAvmBranch = 'a-publier'
$EngineAvmName = 'AvatarManz engine (Belobug/VIctory-Belobug-AMD a-publier = avatarmanz/VIctory-7900XTX + Belobug)'
if ($env:VICTORY_TRY_AVM -eq 'avatarmanz') {
    $EngineAvmUrl = 'https://github.com/avatarmanz/VIctory-7900XTX.git'
    $EngineAvmBranch = 'main'
    $EngineAvmName = 'AvatarManz engine (avatarmanz/VIctory-7900XTX main)'
}
$MinGitUrl = 'https://github.com/git-for-windows/git/releases/download/v2.56.1.windows.1/MinGit-2.56.1-64-bit.zip'
$TryHome = Join-Path $env:LOCALAPPDATA 'VIctoryTry'
$Src = Join-Path $TryHome 'victory'
$Target = Join-Path $TryHome 'target'
$Src8 = Join-Path $TryHome 'victory-8gb'
$Target8 = Join-Path $TryHome 'target-8gb'
$SrcA = Join-Path $TryHome 'victory-avm'
$TargetA = Join-Path $TryHome 'target-avm'
$ConfigPath = Join-Path $TryHome 'config.json'
$LogDir = Join-Path $TryHome 'logs'
$BackupDir = Join-Path $TryHome 'backup'
New-Item -ItemType Directory -Force -Path $TryHome, $LogDir | Out-Null
try { [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12 } catch {}
try { $Host.UI.RawUI.WindowTitle = 'VIctory Try - menu' } catch {}

function Say([string]$msg) { Write-Host ('  ' + $msg) }
function Step([string]$msg) { Write-Host ''; Write-Host ('== ' + $msg) -ForegroundColor Cyan }
function Bad([string]$msg) { Write-Host ('  ' + $msg) -ForegroundColor Red }

function Load-Config {
    # console: series-x, the profile the fork's base (fxceless/victory-gta6)
    # renders the world with on NVIDIA (RTX 3080, 3070 Ti, 4070).
    $c = @{ game_dir = ''; game_exe = ''; built_key = ''; added = ''; console = 'series-x'; base = ''; fast = ''; loop = ''; prepared = ''; recipes = ''; wide = ''; wavewide = ''; new38 = ''; lazy = ''; res = ''; plain = ''; boost = ''; between = ''; hybrid = ''; ambient = ''; levels = ''; opt = ''; shadowfix = ''; gpucopy = ''; defaults = ''; reports = ''; vram = ''; console_auto = ''; console_user = ''; engine = ''; built8 = ''; igpu = ''; igpu_set = ''; rebar = ''; inflight = ''; temporal = ''; save_ask = ''; save_last = ''; slots = ''; fxspeed = ''; low = ''; builta = ''; engine69 = ''; avm_shaders = ''; avm_warm = ''; rt = ''; ports = ''; safe = ''; unlock = ''; texcap = ''; engine610 = ''; fsr_dl = ''; shaders_all = ''; allout = ''; barwin = ''; copyfetch = ''; trim = ''; mig613 = ''; widehash = ''; ahead = ''; standin = ''; tidy = ''; meter = ''; beside = '' }
    if (Test-Path $ConfigPath) {
        try {
            $j = Get-Content -Path $ConfigPath -Raw | ConvertFrom-Json
            foreach ($k in @('game_dir', 'game_exe', 'built_key', 'added', 'console', 'base', 'fast', 'loop', 'prepared', 'recipes', 'wide', 'wavewide', 'new38', 'lazy', 'res', 'plain', 'boost', 'between', 'hybrid', 'ambient', 'levels', 'opt', 'shadowfix', 'gpucopy', 'defaults', 'reports', 'vram', 'console_auto', 'console_user', 'engine', 'built8', 'igpu', 'igpu_set', 'rebar', 'inflight', 'temporal', 'save_ask', 'save_last', 'slots', 'fxspeed', 'low', 'builta', 'engine69', 'avm_shaders', 'avm_warm', 'rt', 'ports', 'safe', 'unlock', 'texcap', 'engine610', 'fsr_dl', 'shaders_all', 'allout', 'barwin', 'copyfetch', 'trim', 'mig613', 'widehash', 'ahead', 'standin', 'tidy', 'meter', 'beside')) { if ($j.$k) { $c[$k] = [string]$j.$k } }
        } catch {}
    }
    # Try 3.0 moved to the victory-gta6 base: start once on Series X.
    if ($c.base -ne 'gta6') { $c.console = 'series-x'; $c.base = 'gta6' }
    # 6.10: this fork's engine once, whatever ran before: it stays the base;
    # the others' fixes are ported into it (N still switches).
    if ($c.engine610 -ne '1') { $c.engine = 'own'; $c.engine610 = '1'; $c.engine69 = '1' }
    return $c
}
function Save-Config { try { ($script:Config | ConvertTo-Json) | Set-Content -Path $ConfigPath -Encoding ASCII } catch {} }

# The user's Downloads folder (wherever Windows has it moved to).
function Downloads-Dir {
    $d = ''
    try {
        $v = (Get-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders' -ErrorAction Stop).'{374DE290-123F-4565-9164-39C4925E467B}'
        if ($v) { $d = [Environment]::ExpandEnvironmentVariables([string]$v) }
    } catch {}
    if (-not $d -or -not (Test-Path -LiteralPath $d)) { $d = Join-Path $env:USERPROFILE 'Downloads' }
    return $d
}
# Where the report zips go (5.4): the folder chosen with S, else a folder
# of their own in Downloads, Downloads\VIctory-reports.
function Reports-Dir {
    $d = "$($script:Config.reports)"
    if (-not $d) { $d = Join-Path (Downloads-Dir) 'VIctory-reports' }
    try { New-Item -ItemType Directory -Force -Path $d -ErrorAction Stop | Out-Null } catch {}
    if (-not (Test-Path -LiteralPath $d)) {
        $d = Join-Path $TryHome 'reports'
        New-Item -ItemType Directory -Force -Path $d -ErrorAction SilentlyContinue | Out-Null
    }
    return $d
}
function Ask-Reports {
    Step 'Where reports are saved'
    Say "now: $(Reports-Dir)"
    $in = Read-Path '  New folder (Enter: the default, Downloads\VIctory-reports)'
    if (-not $in) { $script:Config.reports = ''; Say "reports go to $(Reports-Dir)"; return }
    try {
        New-Item -ItemType Directory -Force -Path $in -ErrorAction Stop | Out-Null
        $script:Config.reports = (Resolve-Path -LiteralPath $in).Path
        Say "reports go to $($script:Config.reports)"
    } catch {
        Bad "could not use '$in' ($_): reports stay in $(Reports-Dir)"
    }
}

function Refresh-Path {
    $env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User')
    $cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
    if ((Test-Path $cargoBin) -and ($env:Path -notlike "*$cargoBin*")) { $env:Path += ";$cargoBin" }
}

function Find-Exe([string]$name) {
    $c = Get-Command $name -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($c) { return $c.Source }
    return $null
}

function Download([string]$url, [string]$file) {
    try { Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $file; return $true }
    catch { Bad "download failed: $url ($_)"; return $false }
}

# ------------------------------------------------------------- the game

function Read-Path([string]$prompt) {
    $in = Read-Host $prompt
    if ($null -eq $in) { return '' }
    return $in.Trim().Trim('"').Trim("'").Trim()
}

# Why `exe` cannot be the game, or '' if it can. VIctory Try itself as the
# "game" made VIctory run VIctory Try, which ran itself again, endlessly.
function Not-Game([string]$exe) {
    $name = [IO.Path]::GetFileName($exe)
    if ($name -like 'VIctoryTry*' -or $name -ieq 'victory.exe' -or $name -ieq 'powershell.exe') {
        return "$name is not the game (it is VIctory Try or VIctory itself)"
    }
    try {
        $full = (Resolve-Path -LiteralPath $exe).ProviderPath
        if ($full.StartsWith($TryHome, [StringComparison]::OrdinalIgnoreCase)) { return "keep the game outside $TryHome" }
        $info = (Get-Item -LiteralPath $full).VersionInfo
        if ($info.ProductName -eq 'VIctory Try' -or $info.FileDescription -eq 'VIctory Try') {
            return "$name is VIctory Try itself, not the game"
        }
    } catch {}
    return ''
}

function Ask-Game([bool]$force) {
    $c = $script:Config
    if (-not $force -and $c.game_exe -and (Test-Path -LiteralPath $c.game_exe -PathType Leaf)) {
        $why = Not-Game $c.game_exe
        if (-not $why) {
            Say "game: $($c.game_exe)"
            if ($c.game_dir) { Say "game files: $($c.game_dir)" }
            return $true
        }
        Bad "the saved game path is wrong: $why"
        $c.game_exe = ''
        Save-Config
    }
    Step 'Where is the game?'
    Say 'Paste each path and press Enter (right-click pastes in this window).'
    Say 'The game files folder is the one with the game''s .rpf archives; the exe may be elsewhere.'
    $dir = ''; $exe = ''
    while ($true) {
        $dir = Read-Path '  1) Game files folder (with the .rpf files)'
        if (-not $dir) { continue }
        if (Test-Path -LiteralPath $dir -PathType Leaf) {
            # The exe was given first: keep it, and still ask for the files.
            $why = Not-Game $dir
            if ($why) { Bad $why; continue }
            $exe = (Resolve-Path -LiteralPath $dir).ProviderPath
            $exeDir = Split-Path -Parent $exe
            Say "game exe: $exe"
            $dir = Read-Path "  1) Game files folder (with the .rpf files) [Enter = $exeDir]"
            if (-not $dir) { $dir = $exeDir }
        }
        if (Test-Path -LiteralPath $dir -PathType Container) { break }
        Bad "not found: $dir"
        $exe = ''
    }
    $dir = (Resolve-Path -LiteralPath $dir).ProviderPath.TrimEnd('\')
    if ($dir.StartsWith($TryHome, [StringComparison]::OrdinalIgnoreCase)) {
        Bad "keep the game outside $TryHome"
        return $false
    }
    $rpfs = @(Get-ChildItem -LiteralPath $dir -Filter '*.rpf' -File -ErrorAction SilentlyContinue)
    if ($rpfs.Count -eq 0) {
        Say 'note: no .rpf files in that folder. If it is not the game files folder, press G later to change it.'
    }
    while (-not $exe) {
        $found = @(Get-ChildItem -LiteralPath $dir -Filter '*.exe' -File -ErrorAction SilentlyContinue | Select-Object -First 9)
        if ($found.Count -gt 1) {
            Say 'exe files in that folder:'
            for ($i = 0; $i -lt $found.Count; $i++) { Say ("  {0}) {1}" -f ($i + 1), $found[$i].Name) }
        }
        $hint = ''
        if ($found.Count -eq 1) { $hint = " [Enter = $($found[0].Name)]" }
        elseif ($found.Count -gt 1) { $hint = ' (a number above, or the path)' }
        $in = Read-Path "  2) Game exe file$hint"
        $pick = ''
        $n = 0
        if (-not $in) {
            if ($found.Count -eq 1) { $pick = $found[0].FullName } else { continue }
        } elseif ([int]::TryParse($in, [ref]$n) -and $n -ge 1 -and $n -le $found.Count) {
            $pick = $found[$n - 1].FullName
        }
        if ($pick) {
            $why = Not-Game $pick
            if ($why) { Bad $why; Say 'pick the game''s own exe (the Xbox title, usually next to MicrosoftGame.config)' }
            else { $exe = $pick }
            continue
        }
        if (-not [IO.Path]::IsPathRooted($in)) { $in = Join-Path $dir $in }
        if (Test-Path -LiteralPath $in -PathType Leaf) { $exe = (Resolve-Path -LiteralPath $in).ProviderPath }
        else { Bad "not found: $in" }
        if ($exe) {
            $why = Not-Game $exe
            if ($why) { Bad $why; Say 'pick the game''s own exe (the Xbox title, usually next to MicrosoftGame.config)'; $exe = '' }
        }
    }
    $c.game_dir = $dir
    $c.game_exe = $exe
    Save-Config
    Say "game files: $dir"
    Say "game exe:   $exe"
    return $true
}

# ------------------------------------------------------------- tools

function Has-Msvc {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path $vswhere)) { return $false }
    $found = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>$null
    return [bool]$found
}

# Git, to fetch the fork and upstream and apply upstream's new commits: the
# installed one, else a portable MinGit kept in our folder.
function Ensure-Git {
    $sys = Find-Exe 'git'
    if ($sys) { $script:Git = $sys; return $true }
    $mine = Join-Path $TryHome 'mingit\cmd\git.exe'
    if (-not (Test-Path $mine)) {
        Step 'Getting a portable Git (first time only, about 40 MB)'
        $zip = Join-Path $TryHome 'mingit.zip'
        if (Download $MinGitUrl $zip) {
            try { Expand-Archive -LiteralPath $zip -DestinationPath (Join-Path $TryHome 'mingit') -Force } catch { Bad "could not unpack Git: $_" }
            Remove-Item -Force $zip -ErrorAction SilentlyContinue
        }
    }
    if (Test-Path $mine) { $script:Git = $mine; return $true }
    Bad 'Git is missing and could not be downloaded. Check the internet connection and start again.'
    return $false
}

# Runs git in the source folder; the exit code lands in $script:GCode.
function G {
    $out = @(& $script:Git -C $Src -c core.autocrlf=false -c advice.detachedHead=false @args 2>&1 | ForEach-Object { "$_" })
    $script:GCode = $LASTEXITCODE
    return $out
}

function Ensure-Tools {
    Refresh-Path
    if (-not (Has-Msvc)) {
        Step 'Installing Microsoft C++ Build Tools (first time only, a few GB)'
        Say 'Windows will ask for permission. A progress window opens; wait for it to finish.'
        $vs = Join-Path $TryHome 'vs_BuildTools.exe'
        if (Download 'https://aka.ms/vs/17/release/vs_BuildTools.exe' $vs) {
            $p = Start-Process -FilePath $vs -ArgumentList '--passive --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended' -Wait -PassThru
            Say "build tools installer finished (code $($p.ExitCode))"
        }
        if (-not (Has-Msvc)) {
            Bad 'the C++ build tools are still missing. Restart Windows and start VIctory Try again.'
            return $false
        }
    }
    if (-not (Ensure-Git)) { return $false }
    if (-not (Find-Exe 'rustup')) {
        Step 'Installing Rust (first time only)'
        $ri = Join-Path $TryHome 'rustup-init.exe'
        if (Download 'https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe' $ri) {
            & $ri -y --profile minimal --default-toolchain none
        }
        Refresh-Path
        if (-not (Find-Exe 'rustup')) {
            Bad 'Rust did not install. Start VIctory Try again; if it keeps failing, install it from https://rustup.rs'
            return $false
        }
    }
    return $true
}

# ------------------------------------------------------------- build

# Fetches the fork and upstream; returns $true when there is something new to
# build, $false when not, $null when there is no source at all.
function Update-Source {
    Step 'Getting the newest VIctory (this fork, plus upstream''s new fixes)'
    if ((Test-Path $Src) -and -not (Test-Path (Join-Path $Src '.git'))) { Remove-Item -Recurse -Force $Src }
    if (-not (Test-Path $Src)) {
        & $script:Git -c core.autocrlf=false clone -q --single-branch --branch $Branch $ForkUrl $Src
        if ($LASTEXITCODE -ne 0) {
            # The fork is gone or unreachable: a mirror, else the newest backup.
            $sources = @($MirrorUrls)
            if (Test-Path $BackupDir) {
                $sources += @(Get-ChildItem -Path $BackupDir -Filter 'catez-*.bundle' |
                    Sort-Object LastWriteTime -Descending | ForEach-Object { $_.FullName })
            }
            foreach ($from in $sources) {
                if (Test-Path $Src) { Remove-Item -Recurse -Force $Src }
                if ($from -like '*.bundle') {
                    & $script:Git init -q $Src
                    & $script:Git -C $Src remote add origin $ForkUrl
                    & $script:Git -C $Src fetch -q $from '+refs/remotes/origin/fork:refs/remotes/origin/fork'
                } else {
                    & $script:Git -c core.autocrlf=false clone -q --single-branch --branch $Branch $from $Src
                }
                if ($LASTEXITCODE -eq 0) { Say "the source came from $from"; break }
            }
            if ($LASTEXITCODE -ne 0) { Bad 'could not download the source: check the internet connection'; return $null }
        }
    }
    $null = G fetch -q origin "+refs/heads/${Branch}:refs/remotes/origin/fork"
    if ($script:GCode -ne 0) {
        foreach ($m in $MirrorUrls) {
            $null = G fetch -q $m "+refs/heads/${Branch}:refs/remotes/origin/fork"
            if ($script:GCode -eq 0) { Say "the fork came from $m"; break }
        }
        if ($script:GCode -ne 0) { Say 'could not reach the fork: using what was downloaded before (and the backups in backup)' }
    }
    Save-Backup
    $null = G fetch -q $UpstreamUrl '+refs/heads/main:refs/remotes/upstream/main'
    if ($script:GCode -ne 0) { Say 'could not reach upstream VIctory: building the fork without its new fixes' }
    $fork = (G rev-parse -q --verify refs/remotes/origin/fork) | Select-Object -First 1
    if (-not $fork) { Bad "the source download is broken: delete the victory folder in $TryHome and start again"; return $null }
    $up = (G rev-parse -q --verify refs/remotes/upstream/main) | Select-Object -First 1
    $key = "fork:$fork+$up"
    if (($key -eq $script:Config.built_key) -and (Test-Path (Ours-Exe))) {
        Say 'no changes since last time'
        return $false
    }
    $script:Key = $key
    return $true
}

# Keeps a copy of the whole fork history on this PC, one file per version
# (backup\catez-<commit>.bundle, the newest 5), so the fork survives even if
# GitHub loses it: `git clone catez-<commit>.bundle CAtez` gives it back.
function Save-Backup {
    $fork = (G rev-parse -q --short=10 --verify refs/remotes/origin/fork) | Select-Object -First 1
    if (-not $fork) { return }
    New-Item -ItemType Directory -Force -Path $BackupDir | Out-Null
    $file = Join-Path $BackupDir "catez-$fork.bundle"
    if (-not (Test-Path $file)) {
        $tmp = "$file.part"
        $null = G bundle create -q $tmp "refs/remotes/origin/fork"
        if ($script:GCode -eq 0 -and (Test-Path $tmp)) {
            Move-Item -Force $tmp $file
            Say "fork backed up: $file"
        } else {
            Remove-Item -Force -ErrorAction SilentlyContinue $tmp
        }
    }
    # (6.16: the newest 2; 5 before. Each is the fork's whole history.)
    Get-ChildItem -Path $BackupDir -Filter 'catez-*.bundle' | Sort-Object LastWriteTime -Descending |
        Select-Object -Skip 2 | Remove-Item -Force -ErrorAction SilentlyContinue
}

# Puts the fork in the source folder, then upstream's commits that came after
# what the fork has (scripts/try/upstream.txt). The added subjects land in
# $script:Added (returned arrays: an empty one counted as one fix).
function Prepare-Source([bool]$withUpstream) {
    $null = G cherry-pick --abort
    $null = G checkout -q -f -B try refs/remotes/origin/fork
    $null = G clean -fdq
    $added = @()
    if (-not $withUpstream) { $script:Added = $added; return }
    $up = (G rev-parse -q --verify refs/remotes/upstream/main) | Select-Object -First 1
    if (-not $up) { $script:Added = $added; return }
    $marker = Join-Path $Src 'scripts\try\upstream.txt'
    $synced = ''; $have = @{}
    if (Test-Path $marker) {
        foreach ($line in Get-Content -LiteralPath $marker) {
            if ($line -match '^\s*synced\s+([0-9a-f]{7,40})') { $synced = $Matches[1] }
            elseif ($line -match '^\s*have\s+([0-9a-f]{7,40})') { $have[$Matches[1]] = $true }
        }
    }
    if (-not $synced) { Say 'the fork does not say which upstream commits it has: not adding any'; $script:Added = $added; return }
    $null = G merge-base --is-ancestor $synced $up
    if ($script:GCode -ne 0) { Say 'upstream history changed: not adding its commits this time'; $script:Added = $added; return }
    $new = @(G rev-list --reverse --first-parent "$synced..$up")
    if ($new.Count -eq 0) { Say 'upstream VIctory has nothing new beyond this fork'; $script:Added = $added; return }
    Say "upstream VIctory has $($new.Count) new commit(s) beyond this fork"
    foreach ($sha in $new) {
        $known = $false
        foreach ($h in $have.Keys) { if ($sha.StartsWith($h)) { $known = $true } }
        $subject = (G log -1 --format=%s $sha) | Select-Object -First 1
        if ($known) { Say "  have already: $subject"; continue }
        $parents = @(((G rev-list --parents -n 1 $sha) | Select-Object -First 1) -split ' ').Count - 1
        $pick = @('-c', 'user.name=VIctory Try', '-c', 'user.email=try@localhost', 'cherry-pick', '-x')
        if ($parents -gt 1) { $pick += @('-m', '1') }
        $null = G @pick $sha
        if ($script:GCode -eq 0) { Say "  added: $subject"; $added += $subject; continue }
        $conflict = @(G diff --name-only --diff-filter=U).Count -gt 0
        $null = G cherry-pick --abort
        $null = G reset -q --hard
        if ($conflict) { Say "  skipped (clashes with this fork's changes): $subject" }
        else { Say "  have already: $subject" }
    }
    $script:Added = $added; return
}

# This fork's build; Victory-Exe: the build that runs, of the chosen engine.
function Ours-Exe { return Join-Path $Target 'release\victory.exe' }
function Engine8-Exe { return Join-Path $Target8 'release\victory.exe' }
# 6.0: this fork's engine again (5.9 made the 8 GB engine the default and it
# made no difference); N picks soldsawn's 8 GB engine ('8gb').
function Engine8 { return $script:Config.engine -eq '8gb' }
function EngineAvm-Exe { return Join-Path $TargetA 'diag\victory.exe' }
function EngineAvm { return $script:Config.engine -eq 'avm' }
# This fork's own engine: neither of the others.
function OwnEngine { return -not (Engine8) -and -not (EngineAvm) }
function Victory-Exe { if (EngineAvm) { return EngineAvm-Exe } elseif (Engine8) { return Engine8-Exe } else { return Ours-Exe } }
function Engine-Name { if (EngineAvm) { return 'avm' } elseif (Engine8) { return '8GB' } else { return 'own' } }

function Build {
    Step 'Building VIctory (the first build takes a while; later ones are quicker)'
    $channel = ''
    $tc = Join-Path $Src 'rust-toolchain.toml'
    if (Test-Path $tc) {
        $m = Select-String -LiteralPath $tc -Pattern '^\s*channel\s*=\s*"([^"]+)"' | Select-Object -First 1
        if ($m) { $channel = $m.Matches[0].Groups[1].Value }
    }
    if ($channel) {
        Say "Rust $channel"
        & rustup toolchain install $channel --profile minimal --target x86_64-pc-windows-msvc 2>&1 | ForEach-Object { "$_" } | Out-Host
    }
    $env:CARGO_TARGET_DIR = $Target
    $log = Join-Path $LogDir 'build.log'
    # 6.0: built for THIS PC's CPU (AVX2 / AVX-512 where it has them: the
    # copies, compares, hashing and detiling are most of a world frame) and
    # with thin link-time optimisation across the crates (fxceless #1117).
    # The first build after this takes longer (everything is built again
    # once). If it fails, the plain build is tried. VICTORY_TRY_PLAIN_BUILD=1:
    # the plain build only.
    $tuned = $env:VICTORY_TRY_PLAIN_BUILD -ne '1'
    Push-Location $Src
    try {
        foreach ($pass in @($(if ($tuned) { 'tuned' }), 'plain')) {
            if (-not $pass) { continue }
            if ($pass -eq 'tuned') {
                Say 'build: tuned for this CPU (target-cpu=native) with thin LTO'
                $env:RUSTFLAGS = '-C target-cpu=native'
                $env:CARGO_PROFILE_RELEASE_LTO = 'thin'
            } else {
                Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue
                Remove-Item Env:CARGO_PROFILE_RELEASE_LTO -ErrorAction SilentlyContinue
            }
            # Out-Host: the lines must not become this function's result (they
            # did up to Try 4.0, so a failed build counted as built and the old
            # victory.exe ran on).
            & cargo build --release -p victory 2>&1 | ForEach-Object { "$_" } | Tee-Object -FilePath $log | Out-Host
            $code = $LASTEXITCODE
            if ($code -eq 0) { break }
            if ($pass -eq 'tuned') { Bad 'the tuned build failed; trying the plain build' }
        }
    } finally {
        Pop-Location
        Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue
        Remove-Item Env:CARGO_PROFILE_RELEASE_LTO -ErrorAction SilentlyContinue
    }
    if ($code -ne 0 -or -not (Test-Path (Ours-Exe))) {
        Bad "the build failed (code $code). The full output is in $log"
        $lines = @(Get-Content -LiteralPath $log -ErrorAction SilentlyContinue)
        $first = -1
        for ($i = 0; $i -lt $lines.Count; $i++) { if ($lines[$i] -match '^error') { $first = $i; break } }
        if ($first -lt 0) { $first = [Math]::Max(0, $lines.Count - 25) }
        Say 'the first error (paste this when reporting it):'
        $lines[$first..([Math]::Min($lines.Count - 1, $first + 24))] | ForEach-Object { Write-Host ('    ' + $_) }
        return $false
    }
    Say 'build finished'
    return $true
}

# Whether a Build call succeeded: its last result is $true (anything else
# it lets slip out of a command comes before).
function Built { $r = @($input); return ($r.Count -gt 0) -and ($r[-1] -eq $true) }

# The fork alone; with VICTORY_TRY_UPSTREAM=1 upstream's new fixes on top
# first, and if that does not build, the fork alone. (Up to Try 4.0 they
# were always added: one of them, upstream #1180, does not build with this
# fork's shader code.)
# ------------------------------------------------------------- the disk
# 6.16: why every new version took more of the disk, and what goes now.
# - The shader cache (cache-gta6\shaders): each build of the translator
#   translated all 25,746 of the game's shaders again (about a gigabyte) and
#   kept them beside every older build's, which nothing read again. VIctory
#   itself now keeps the two most recently used builds' translations and
#   removes the rest a minute and a half into a run or a prepare
#   (VICTORY_SHADER_CACHE_KEEP; the 6.15 cache's files go once).
# - The build folder (target): cargo never removes the pieces older builds
#   made. Past 12 GB (VICTORY_TRY_TARGET_MAX_GB) it is emptied before the
#   next build, which then takes longer once.
# - The other engines (victory-avm, target-avm, cache-avm, victory-8gb,
#   target-8gb, cache-8gb): removed while that engine is not the one in use
#   (N builds it again).
# - Game logs: the newest 10; fork backups: the newest 2; failed shaders
#   older than 3 days; half-made report folders; a refused pipeline library.
#   (Report zips in Downloads are left to you.)
# config.json tidy=off: nothing is removed. TIDY shows what takes what.
function Folder-Mb([string]$path) {
    if (-not $path -or -not (Test-Path -LiteralPath $path)) { return [int64]0 }
    $sum = (Get-ChildItem -LiteralPath $path -Recurse -File -Force -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
    if ($null -eq $sum) { return [int64]0 }
    return [int64]($sum / 1MB)
}
function Remove-Folder([string]$path, [string]$why) {
    if (-not $path -or -not (Test-Path -LiteralPath $path)) { return [int64]0 }
    $mb = Folder-Mb $path
    Remove-Item -LiteralPath $path -Recurse -Force -ErrorAction SilentlyContinue
    if (Test-Path -LiteralPath $path) {
        $left = Folder-Mb $path
        Bad "disk: could not remove all of $path ($left MB left; in use?)"
        return [int64]($mb - $left)
    }
    Say "disk: removed $path ($mb MB): $why"
    return $mb
}
function Keep-Newest([string]$dir, [string]$filter, [int]$n) {
    if (-not $dir -or -not (Test-Path -LiteralPath $dir)) { return [int64]0 }
    $old = @(Get-ChildItem -LiteralPath $dir -Filter $filter -File -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -Skip $n)
    $bytes = [int64]0
    foreach ($f in $old) {
        $bytes += $f.Length
        Remove-Item -LiteralPath $f.FullName -Force -ErrorAction SilentlyContinue
    }
    return [int64]($bytes / 1MB)
}
function Target-MaxGb {
    $v = 0
    if ([int]::TryParse("$env:VICTORY_TRY_TARGET_MAX_GB", [ref]$v) -and $v -gt 0) { return $v }
    return 12
}
# Before a build of this fork's engine: an overgrown build folder is emptied.
function Tidy-Target {
    if ($script:Config.tidy -eq 'off' -or -not (Test-Path -LiteralPath $Target)) { return }
    $mb = Folder-Mb $Target
    $max = Target-MaxGb
    if ($mb -gt ($max * 1024)) {
        Say ("disk: the build folder holds {0:N1} GB (cargo keeps every older build's pieces): emptied before this build (over {1} GB, VICTORY_TRY_TARGET_MAX_GB); this build takes longer once" -f ($mb / 1024), $max)
        $null = Remove-Folder $Target 'older builds'' pieces'
    }
}
function Tidy-Disk([bool]$sizes) {
    $c = $script:Config
    if ($c.tidy -eq 'off') {
        if ($sizes) { Say 'disk: tidy=off in config.json: nothing is removed' }
        return
    }
    $freed = [int64]0
    if (-not (EngineAvm)) {
        foreach ($p in @($SrcA, $TargetA, (Join-Path $TryHome 'cache-avm'))) { $freed += Remove-Folder $p 'the AvatarManz engine is not in use (N builds it again)' }
    }
    if (-not (Engine8)) {
        foreach ($p in @($Src8, $Target8, (Join-Path $TryHome 'cache-8gb'))) { $freed += Remove-Folder $p 'the 8 GB engine is not in use (N builds it again)' }
    }
    $freed += Keep-Newest $LogDir 'game-*.log' 10
    $freed += Keep-Newest $BackupDir 'catez-*.bundle' 2
    foreach ($d in @(Get-ChildItem -LiteralPath $TryHome -Directory -Filter 'report-*' -ErrorAction SilentlyContinue)) { $freed += Remove-Folder $d.FullName 'a report folder left half made' }
    $rejected = Join-Path $TryHome 'cache-gta6\pipelines\host-pipelines.rejected.bin'
    if (Test-Path -LiteralPath $rejected) {
        $freed += [int64]((Get-Item -LiteralPath $rejected).Length / 1MB)
        Remove-Item -LiteralPath $rejected -Force -ErrorAction SilentlyContinue
    }
    if ($freed -gt 0) { Say ("disk: {0:N2} GB freed" -f ($freed / 1024)) }
    if ($sizes) {
        $cache = Join-Path $TryHome 'cache-gta6'
        $parts = [ordered]@{
            'build (target)' = Folder-Mb $Target
            'shader cache' = Folder-Mb (Join-Path $cache 'shaders')
            'pipelines' = Folder-Mb (Join-Path $cache 'pipelines')
            'source' = Folder-Mb $Src
            'logs' = Folder-Mb $LogDir
            'backups' = Folder-Mb $BackupDir
        }
        $all = Folder-Mb $TryHome
        $line = (@($parts.Keys | ForEach-Object { '{0} {1:N1}' -f $_, ($parts[$_] / 1024) }) -join ', ')
        Say ("disk: VIctory Try takes {0:N1} GB in {1}: {2} (GB). Older builds' shader translations go during the next run." -f ($all / 1024), $TryHome, $line)
    }
}

function Build-All {
    Tidy-Target
    $null = Prepare-Source ($env:VICTORY_TRY_UPSTREAM -eq '1')
    $added = @($script:Added | Where-Object { $_ })
    $ok = Build | Built
    if (-not $ok -and $added.Count -gt 0) {
        Bad "the build failed with upstream's new fixes added: building this fork without them"
        $added = @()
        $null = Prepare-Source $false
        $ok = Build | Built
    }
    if ($ok) {
        $script:Config.built_key = $script:Key
        $script:Config.added = ($added -join "`n")
        Save-Config
        if ($added.Count -gt 0) { Say "this build includes $($added.Count) new upstream fix(es)" }
    }
    return $ok
}

# ------------------------------------------------------------- 8 GB engine

function G8 {
    $out = @(& $script:Git -C $Src8 -c core.autocrlf=false -c advice.detachedHead=false @args 2>&1 | ForEach-Object { "$_" })
    $script:GCode = $LASTEXITCODE
    return $out
}

# Downloads the 8 GB engine (or its news). $true: new, to be built; $false:
# built already; $null: nothing to build.
function Update-Engine8 {
    Step 'Getting the 8 GB NVIDIA engine (soldsawn/victory-nvidia-8gb: renders on an RTX 4060 8 GB)'
    if ((Test-Path $Src8) -and -not (Test-Path (Join-Path $Src8 '.git'))) { Remove-Item -Recurse -Force $Src8 }
    if (-not (Test-Path $Src8)) {
        & $script:Git -c core.autocrlf=false clone -q --single-branch --branch $Engine8Branch $Engine8Url $Src8
        if ($LASTEXITCODE -ne 0) { Bad 'could not download the 8 GB engine: check the internet connection'; return $null }
    }
    $null = G8 fetch -q origin "+refs/heads/${Engine8Branch}:refs/remotes/origin/engine"
    if ($script:GCode -ne 0) { Say 'could not reach the 8 GB engine: using what was downloaded before' }
    $head = (G8 rev-parse -q --verify refs/remotes/origin/engine) | Select-Object -First 1
    if (-not $head) { Bad "the 8 GB engine download is broken: delete $Src8 and press U"; return $null }
    $script:Key8 = "engine8:$head"
    if (($script:Key8 -eq $script:Config.built8) -and (Test-Path (Engine8-Exe))) { Say 'no changes since last time'; return $false }
    return $true
}

function Build-Engine8 {
    $null = G8 checkout -q -f -B try refs/remotes/origin/engine
    $null = G8 clean -fdq
    $line = (G8 log -1 --format='%h %ad %s' --date=short HEAD) | Select-Object -First 1
    Step 'Building the 8 GB NVIDIA engine (the first build takes a while; later ones are quicker)'
    Say "engine: $line"
    $tc = Join-Path $Src8 'rust-toolchain.toml'
    if (Test-Path $tc) {
        $m = Select-String -LiteralPath $tc -Pattern '^\s*channel\s*=\s*"([^"]+)"' | Select-Object -First 1
        if ($m) {
            $channel = $m.Matches[0].Groups[1].Value
            Say "Rust $channel"
            & rustup toolchain install $channel --profile minimal --target x86_64-pc-windows-msvc 2>&1 | ForEach-Object { "$_" } | Out-Host
        }
    }
    $env:CARGO_TARGET_DIR = $Target8
    $log = Join-Path $LogDir 'build.log'
    Push-Location $Src8
    try {
        & cargo build --release -p victory 2>&1 | ForEach-Object { "$_" } | Tee-Object -FilePath $log | Out-Host
        $code = $LASTEXITCODE
    } finally { Pop-Location; $env:CARGO_TARGET_DIR = $Target }
    if ($code -ne 0 -or -not (Test-Path (Engine8-Exe))) {
        Bad "the 8 GB engine did not build (code $code). The full output is in $log"
        $lines = @(Get-Content -LiteralPath $log -ErrorAction SilentlyContinue)
        $first = -1
        for ($i = 0; $i -lt $lines.Count; $i++) { if ($lines[$i] -match '^error') { $first = $i; break } }
        if ($first -lt 0) { $first = [Math]::Max(0, $lines.Count - 25) }
        $lines[$first..([Math]::Min($lines.Count - 1, $first + 24))] | ForEach-Object { Write-Host ('    ' + $_) }
        return $false
    }
    $script:Config.built8 = $script:Key8
    Save-Config
    Say 'build finished'
    return $true
}

# Gets and builds the chosen engine. With the 8 GB engine: if it cannot be
# had (no download, no build, none built before), this fork's engine runs.
function Ensure-Engine([bool]$update) {
    if (OwnEngine) {
        if ($update -or -not (Test-Path (Ours-Exe))) { return (Build-All) }
        return $true
    }
    if (EngineAvm) {
        $new = $true
        if ($update -or -not (Test-Path (EngineAvm-Exe))) { $new = Update-EngineAvm } else { $new = $false }
        if ($new) { $null = Build-EngineAvm }
        if (Test-Path (EngineAvm-Exe)) { return $true }
        Bad 'no AvatarManz engine to run: running this fork''s engine instead (N switches engines)'
        $script:Config.engine = 'own'
        Save-Config
        if (-not (Test-Path (Ours-Exe))) { return (Build-All) }
        return $true
    }
    $new = $true
    if ($update -or -not (Test-Path (Engine8-Exe))) { $new = Update-Engine8 } else { $new = $false }
    if ($new) { $null = Build-Engine8 }
    if (Test-Path (Engine8-Exe)) { return $true }
    Bad 'no 8 GB engine to run: running this fork''s engine instead (N switches engines)'
    $script:Config.engine = 'own'
    Save-Config
    if (-not (Test-Path (Ours-Exe))) { return (Build-All) }
    return $true
}

# The 8 GB engine's settings: its launcher's (scripts/play-nvidia-8gb.ps1),
# none of this fork's (their names mean other things there, or nothing),
# Series S and smaller card budgets on cards under 7 GB. What the user set
# before VIctory Try started stays.
function Set-Engine8Env {
    $c = $script:Config
    Get-ChildItem Env: | Where-Object { $_.Name -like 'VICTORY_*' } | ForEach-Object { Remove-Item -LiteralPath "Env:$($_.Name)" -ErrorAction SilentlyContinue }
    foreach ($k in $script:UserAll.Keys) { [Environment]::SetEnvironmentVariable($k, $script:UserAll[$k], 'Process') }
    $card = Card-Vram
    $small = ($card -gt 0 -and $card -lt 7000) -or ($c.vram -and [int]$c.vram -lt 6000)
    $set = [ordered]@{
        VICTORY_GAME_DIR = $c.game_dir
        VICTORY_CACHE_DIR = (Join-Path $TryHome 'cache-8gb')
        VICTORY_DUMP_FAILED = (Join-Path $TryHome 'failed-shaders')
        VICTORY_RESOLUTION = $(if ($c.res -eq '4k') { '3840x2160' } else { '1920x1080' })
        VICTORY_TEXTURE_DROP_MIPS = '2'
        VICTORY_COPY_PLAIN = '1'
        VICTORY_INDIRECT_GATHER = '1'
        VICTORY_HEAP_WORKER = '1'
        VICTORY_ALIAS_ONCE = '1'
        VICTORY_JOIN_PARTS = '1'
        VICTORY_BACK_OWN_VIEW = '1'
        VICTORY_INDIRECT_WINDOW_DRAWS = '1'
        VICTORY_GUARD_HOLES_FRAME = '1'
        VICTORY_COPY_CHANGED = '1'
        VICTORY_CARD_WINDOWS_MB = $(if ($small) { '1024' } else { '2048' })
        VICTORY_CARD_BUFFERS_MB = $(if ($small) { '768' } else { '1024' })
        VICTORY_CLAMP_WAITS = '0'
        VICTORY_WIDE_MERGE = '1'
        VICTORY_RELAY_OWN_QUEUE = '1'
    }
    foreach ($k in $set.Keys) { if ($null -eq $script:UserAll[$k]) { [Environment]::SetEnvironmentVariable($k, [string]$set[$k], 'Process') } }
    Say "engine: 8 GB NVIDIA (soldsawn/victory-nvidia-8gb)   (N switches to this fork's engine)"
    Say "its settings: textures 2 sizes smaller (VICTORY_TEXTURE_DROP_MIPS=$($env:VICTORY_TEXTURE_DROP_MIPS)), screen $($env:VICTORY_RESOLUTION), texture upload thread, card budgets $($env:VICTORY_CARD_WINDOWS_MB)+$($env:VICTORY_CARD_BUFFERS_MB) MB"
    Say 'F10 in the game shows its performance overlay.'
}


# ------------------------------------------------------------- AvatarManz engine (6.9)

function GA {
    $out = @(& $script:Git -C $SrcA -c core.autocrlf=false -c advice.detachedHead=false @args 2>&1 | ForEach-Object { "$_" })
    $script:GCode = $LASTEXITCODE
    return $out
}

# Downloads the AvatarManz engine (or its news). $true: new, to be built;
# $false: built already; $null: nothing to build.
function Update-EngineAvm {
    Step "Getting the $EngineAvmName"
    if ((Test-Path $SrcA) -and -not (Test-Path (Join-Path $SrcA '.git'))) { Remove-Item -Recurse -Force $SrcA }
    if (-not (Test-Path $SrcA)) {
        & $script:Git -c core.autocrlf=false clone -q --single-branch --branch $EngineAvmBranch $EngineAvmUrl $SrcA
        if ($LASTEXITCODE -ne 0) { Bad 'could not download the AvatarManz engine: check the internet connection'; return $null }
    }
    # (VICTORY_TRY_AVM may have changed which repository it is.)
    $null = GA remote set-url origin $EngineAvmUrl
    $null = GA fetch -q origin "+refs/heads/${EngineAvmBranch}:refs/remotes/origin/engine"
    if ($script:GCode -ne 0) { Say 'could not reach the AvatarManz engine: using what was downloaded before' }
    $head = (GA rev-parse -q --verify refs/remotes/origin/engine) | Select-Object -First 1
    if (-not $head) { Bad "the AvatarManz engine download is broken: delete $SrcA and press U"; return $null }
    $script:KeyA = "engineavm:$head"
    if (($script:KeyA -eq $script:Config.builta) -and (Test-Path (EngineAvm-Exe))) { Say 'no changes since last time'; return $false }
    return $true
}

# Their own build command (README "How to run it"): the diag profile with
# the diagnostics feature, target\diag\victory.exe. Built for this PC's CPU
# first (as this fork's engine), plainly if that fails.
function Build-EngineAvm {
    $null = GA checkout -q -f -B try refs/remotes/origin/engine
    $null = GA clean -fdq
    $line = (GA log -1 --format='%h %ad %s' --date=short HEAD) | Select-Object -First 1
    Step 'Building the AvatarManz engine (the first build takes a while; later ones are quicker)'
    Say "engine: $line"
    $tc = Join-Path $SrcA 'rust-toolchain.toml'
    if (Test-Path $tc) {
        $m = Select-String -LiteralPath $tc -Pattern '^\s*channel\s*=\s*"([^"]+)"' | Select-Object -First 1
        if ($m) {
            $channel = $m.Matches[0].Groups[1].Value
            Say "Rust $channel"
            & rustup toolchain install $channel --profile minimal --target x86_64-pc-windows-msvc 2>&1 | ForEach-Object { "$_" } | Out-Host
        }
    }
    $env:CARGO_TARGET_DIR = $TargetA
    $log = Join-Path $LogDir 'build.log'
    $oldFlags = $env:RUSTFLAGS
    $code = 1
    Push-Location $SrcA
    try {
        foreach ($pass in @($(if ($env:VICTORY_TRY_PLAIN_BUILD -ne '1') { 'tuned' }), 'plain')) {
            if (-not $pass) { continue }
            $env:RUSTFLAGS = $(if ($pass -eq 'tuned') { '-C target-cpu=native' } else { $oldFlags })
            Say "build: $pass"
            & cargo build --locked --profile diag -p victory --features diagnostics 2>&1 | ForEach-Object { "$_" } | Tee-Object -FilePath $log | Out-Host
            $code = $LASTEXITCODE
            if ($code -eq 0 -and (Test-Path (EngineAvm-Exe))) { break }
        }
    } finally { Pop-Location; $env:CARGO_TARGET_DIR = $Target; $env:RUSTFLAGS = $oldFlags }
    if ($code -ne 0 -or -not (Test-Path (EngineAvm-Exe))) {
        Bad "the AvatarManz engine did not build (code $code). The full output is in $log"
        $lines = @(Get-Content -LiteralPath $log -ErrorAction SilentlyContinue)
        $first = -1
        for ($i = 0; $i -lt $lines.Count; $i++) { if ($lines[$i] -match '^error') { $first = $i; break } }
        if ($first -lt 0) { $first = [Math]::Max(0, $lines.Count - 25) }
        $lines[$first..([Math]::Min($lines.Count - 1, $first + 24))] | ForEach-Object { Write-Host ('    ' + $_) }
        return $false
    }
    $script:Config.builta = $script:KeyA
    $script:Config.avm_shaders = ''
    $script:Config.avm_warm = ''
    Save-Config
    Say 'the AvatarManz engine is built'
    return $true
}

# The PC's memory in MB: [RAM, RAM + page file (the commit limit)], or 0s.
function Ram-Info {
    try {
        $os = Get-CimInstance Win32_OperatingSystem -ErrorAction Stop
        return @([int]($os.TotalVisibleMemorySize / 1024), [int]($os.TotalVirtualMemorySize / 1024))
    } catch { return @(0, 0) }
}

# Its settings: AvatarManz's scripts/play.ps1 (the ones its 45 ms frames
# were measured with), its five renderer patches on (each one compares the
# title's bytes before it writes; 47.3 -> 44.9 ms together), and this PC's
# card. None of this fork's settings (their names mean other things there).
# What the user set before VIctory Try started stays.
function Set-EngineAvmEnv {
    $c = $script:Config
    Get-ChildItem Env: | Where-Object { $_.Name -like 'VICTORY_*' } | ForEach-Object { Remove-Item -LiteralPath "Env:$($_.Name)" -ErrorAction SilentlyContinue }
    foreach ($k in $script:UserAll.Keys) { [Environment]::SetEnvironmentVariable($k, $script:UserAll[$k], 'Process') }
    $env:RUST_BACKTRACE = '1'
    $set = [ordered]@{
        VICTORY_TRY_VERSION = $TryVersion
        VICTORY_GAME_DIR = $c.game_dir
        VICTORY_CACHE_DIR = (Join-Path $TryHome 'cache-avm')
        # play.ps1, all of it:
        VICTORY_TEXTURE_NO_WAIT = '1'; VICTORY_RELAY = '2'; VICTORY_WIDE_BACK = '1'; VICTORY_CARD_REHASH_RUNS = '1'
        VICTORY_JOIN_COPIES = '1'; VICTORY_INDIRECT_WINDOW = '1'; VICTORY_RELAY_COPY = '1'; VICTORY_CARD_STEPS = '2'
        VICTORY_INDIRECT_BATCH = '2'; VICTORY_MAP_THREADS = '1'; VICTORY_TITLE_PRIORITIES = '0'; VICTORY_CARD_AWAKE = '1'
        VICTORY_SHOW_AFTER = '1'; VICTORY_D3D12_DRED = 'light'
        # Every pipeline the title creates is written down, so that
        # "victory warm --used" makes them with the game closed (no more
        # pipelines compiled while you play after the first run of a place).
        VICTORY_PIPELINE_BOOK = '1'
    }
    # A (ambient): play.ps1's 0.8; low / high / off.
    $set.VICTORY_AMBIENT = $(switch ($c.ambient) { 'low' { '0.5' } 'high' { '1.1' } 'off' { '0' } default { '0.8' } })
    # Z: play.ps1 -Safe (without the newest, faster ways) or the fast set.
    if ($c.safe -eq '1') { $set.VICTORY_UNDER_WIDE = '0'; $set.VICTORY_LAND_ECHO = '0' }
    else { $set.VICTORY_THIN = '1'; $set.VICTORY_JOIN_PARTS = '8'; $set.VICTORY_UPLOAD_AHEAD = '1' }
    # Y: renderer patches 1, 2, 5, 6, 7 (README: 47.3 -> 44.9 ms a frame).
    if ($c.ports -ne 'off') {
        foreach ($k in @('VICTORY_PORT_CONSTANTS', 'VICTORY_PORT_DESCRIPTORS', 'VICTORY_PORT_CBV', 'VICTORY_PORT_GUARD', 'VICTORY_PORT_JOURNAL')) { $set[$k] = '1' }
    }
    # B: the title's ray-traced indirect light (play.ps1 -RayTracing): lit
    # interiors and right reflections, about a third slower. Off by default.
    if ($c.rt -eq '1') { $set.VICTORY_BVH_BUILDER = '1' }
    # F: frames above 30 (menus and videos reach 60; the game's clock stays right).
    if ($c.unlock -eq '1') { $set.VICTORY_FRAME_INTERVAL_MS = '16.667' }
    # X: textures. Belobug's default follows the card's free memory (1024,
    # then 512 from 92 % and 256 from 97 % of its budget): what keeps a 6 GB
    # card from overflowing, but faces get blurry when it is full. sharp: up to
    # 2048 whatever the memory; low: 512 always (6 GB cards that stutter).
    $set.VICTORY_TEXTURE_MAX = $(switch ($c.texcap) { 'sharp' { '2048' } 'low' { '512' } default { $null } })
    # R on Series X: the title's own output size (VICTORY_OUTPUT; the title
    # ignores the display mode). Series S outputs 1080p by itself.
    if ($c.console -eq 'series-x') { $set.VICTORY_OUTPUT = $(if ($c.res -eq '4k') { '3840x2160' } else { '1920x1080' }) }
    # Under 32 GB of RAM: the copies of title memory windows (7 GB of its
    # 40-46 GB in AvatarManz's findings) are capped at 1.5 GB once the PC's
    # memory gets tight (its "Watch" and "High" RAM bands; nothing changes
    # while there is room).
    $ram = Ram-Info
    if ($ram[0] -gt 0 -and $ram[0] -lt 32000) { $set.VICTORY_WINDOW_BUDGET_MB = '1536' }
    foreach ($k in $set.Keys) {
        if ($null -eq $script:UserAll[$k] -and $null -ne $set[$k]) { [Environment]::SetEnvironmentVariable($k, [string]$set[$k], 'Process') }
    }
    Say "engine: $EngineAvmName   (N switches engines)"
    Say "its settings: AvatarManz's play.ps1$(if ($c.safe -eq '1') { ' -Safe' }), renderer patches $(if ($c.ports -ne 'off') { 'on' } else { 'off' }) (Y), ray tracing $(if ($c.rt -eq '1') { 'on' } else { 'off' }) (B), textures $(if ($c.texcap) { $c.texcap } else { 'by free card memory' }) (X), ambient $($env:VICTORY_AMBIENT) (A)"
    Say 'F3 in the game shows its panel of numbers (where a frame goes).'
    if ($ram[0] -gt 0) {
        Say "memory: $([int]($ram[0] / 1024)) GB of RAM, $([int]($ram[1] / 1024)) GB with the page file"
        if ($ram[1] -lt 40000) {
            Bad "AvatarManz's engine takes 30-46 GB of memory (RAM + page file) for now. This PC can give $([int]($ram[1] / 1024)) GB:"
            Bad 'if the game closes by itself, set the page file to "System managed" or 49152 MB on your fastest drive'
            Bad '(Settings > System > About > Advanced system settings > Performance > Advanced > Virtual memory), restart, run again.'
        }
    }
}

# With the game closed: every shader translated ("victory shaders", once per
# build: a few minutes) and every pipeline earlier runs drew with made
# ("victory warm --used", quick when there is nothing new). The game then
# compiles nothing it has met before while you play.
function Prepare-Avm([string]$why) {
    $c = $script:Config
    $exe = EngineAvm-Exe
    $pdir = Join-Path $env:VICTORY_CACHE_DIR 'pipelines'
    $log = Join-Path $LogDir 'prepare.log'
    $cores = [Environment]::ProcessorCount
    if ("$($c.avm_shaders)" -ne "$($c.builta)") {
        Step "Translating every shader of the game with it closed ($why)"
        Say 'Once per build: a few minutes. Stopping it is fine (the game translates the rest itself).'
        Push-Location $TryHome
        try {
            & $exe shaders --jobs ([Math]::Max(2, $cores - 1)) 2>&1 | ForEach-Object { "$_" } | Tee-Object -FilePath $log | ForEach-Object { Say $_ }
            $code = $LASTEXITCODE
        } finally { Pop-Location }
        if ($code -eq 0) { Say 'every shader translated' } else { Bad "translating stopped (code $code; output in $log)" }
        $c.avm_shaders = $c.builta
        Save-Config
    }
    $early = Join-Path $pdir 'early-pipelines.txt'
    $stamp = "$($c.builta)|$(if (Test-Path -LiteralPath $early) { (Get-Item -LiteralPath $early).Length } else { 0 })"
    if ((Test-Path -LiteralPath $early) -and "$($c.avm_warm)" -ne $stamp) {
        Step 'Making the pipelines earlier runs drew with, with the game closed'
        Say 'Quick when there is nothing new; the first time after a build or driver update takes longer.'
        # (Each worker is a process with its own compiler: a third of the
        # threads, so 16 GB PCs keep their memory.)
        Push-Location $TryHome
        try {
            & $exe warm --used --jobs ([Math]::Max(2, [int]($cores / 3))) 2>&1 | ForEach-Object { "$_" } | Tee-Object -FilePath $log -Append | ForEach-Object { Say $_ }
            $code = $LASTEXITCODE
        } finally { Pop-Location }
        if ($code -eq 0) { Say 'pipelines made' } else { Bad "making pipelines stopped (code $code; output in $log)" }
        $c.avm_warm = $stamp
        Save-Config
    }
}

# ------------------------------------------------------------- run

# The settings a run uses, also for Prepare-Shaders (the shader cache keys
# its translations by some of them: the same ones are needed both times).
# 6.10: AMD's signed PC upscaler (FidelityFX SDK 2.4.0, MIT), downloaded
# once from AMD's GitHub (57 MB) into the VIctory Try folder. Its folder,
# or $null when it cannot be had (the game then uses our own upscaler).
$FsrUrl = 'https://raw.githubusercontent.com/GPUOpen-LibrariesAndSDKs/FidelityFX-SDK/v2.4.0/Kits/FidelityFX/signedbin/amd_fidelityfx_upscaler_dx12.dll'
$FsrSize = 57664272
function Ensure-Fsr {
    $dir = Join-Path $TryHome 'fidelityfx'
    $dll = Join-Path $dir 'amd_fidelityfx_upscaler_dx12.dll'
    $ok = { (Test-Path -LiteralPath $dll) -and ((Get-Item -LiteralPath $dll).Length -eq $FsrSize) }
    if (-not (& $ok)) {
        if ($script:Config.fsr_dl -eq 'failed' -and -not $script:FsrRetry) { return $null }
        $script:FsrRetry = $false
        Step 'Downloading AMD FidelityFX upscaler (57 MB, once)'
        try { New-Item -ItemType Directory -Force -Path $dir | Out-Null } catch {}
        $part = "$dll.part"
        $got = Download $FsrUrl $part
        if ($got -and (Test-Path -LiteralPath $part) -and ((Get-Item -LiteralPath $part).Length -eq $FsrSize)) {
            Move-Item -Force -LiteralPath $part -Destination $dll
        } else {
            Remove-Item -Force -LiteralPath $part -ErrorAction SilentlyContinue
            Bad 'could not download AMD''s upscaler: VIctory''s own runs instead (T, or U, tries again)'
            $script:Config.fsr_dl = 'failed'; Save-Config
            return $null
        }
    }
    # AMD's signature, checked every run (a damaged or replaced file is not loaded).
    try {
        $sig = Get-AuthenticodeSignature -LiteralPath $dll
        if ($sig.Status -ne 'Valid' -or "$($sig.SignerCertificate.Subject)" -notmatch 'Advanced Micro Devices') {
            Bad "AMD's upscaler is not signed by AMD ($($sig.Status)): not used"
            Remove-Item -Force -LiteralPath $dll -ErrorAction SilentlyContinue
            return $null
        }
    } catch { Bad "could not check AMD's upscaler signature: not used ($_)"; return $null }
    if ($script:Config.fsr_dl -ne 'ok') { $script:Config.fsr_dl = 'ok'; Save-Config }
    return $dir
}
# 6.12: all-out mode (MAX, or VICTORY_TRY_ALL_OUT=1): the caps VICTORY puts
# on itself are lifted to what this PC has, less about 1% (and one CPU
# thread) so Windows, the mouse and the keyboard still answer. Read from the
# PC each run: the logical processors, the memory and the card's memory.
# What it cannot do: give the game more memory than the PC has. VICTORY's
# memory-pressure guard then starts later (about 1% of the memory free
# instead of 1 GB), so it keeps uploading where it used to hold back; if the
# PC is short, Windows pages, and a card that is full can be lost.
function AllOut { return ($script:Config.allout -eq '1' -or $env:VICTORY_TRY_ALL_OUT -eq '1') -and $script:Config.plain -ne '1' }
function Set-AllOut {
    $c = $script:Config
    $threads = [Environment]::ProcessorCount
    $use = [Math]::Max(1, $threads - 1)
    $ram = Ram-Info
    $ramMb = [int64]$ram[0]; $virtMb = [int64]$ram[1]
    $vram = $(if ($c.vram) { [int]$c.vram } else { Card-Vram })
    $ramCrit = [Math]::Max(128, [int]($ramMb / 100))
    $commitCrit = [Math]::Max(256, [int]($virtMb / 100))
    $cardMb = [Math]::Max(1024, [int]($vram / 5))
    # 6.16: on cards under 8 GB, textures up to 94% of the card and the card's
    # pressure at 95/98%. At 99% (the 6.15 report, 6 GB RTX 4050 Laptop) the
    # card stood at 5,050-5,100 MB of its 5,153 MB budget for the whole world
    # and the game's memory windows found no room on it: shaders read them
    # from the PC's memory across the bus (about 200 ms of card work a frame).
    $small = $vram -gt 0 -and $vram -lt 8000
    # 6.20: and on those cards the buffers compared on the card stay at the
    # engine's own 512 MB (cards up to 14 GB), and the memory windows through the BAR at 20% of
    # the card. With a fifth of the card (1,031 MB) and 35% (1,804 MB) the
    # 6.18 report's 6 GB RTX 4050 Laptop stood at 5,716 MB of its 5,153 MB
    # budget: 1,805 MB of buffer copies and 1,148 MB of windows left the
    # textures no room ("textures picked by index: the card has no room"),
    # and what is past the budget Windows moves to the PC's memory.
    if ($small) { $cardMb = 512 }
    $texRoom = $(if ($small) { '94' } else { '99' })
    $cardPressure = $(if ($small) { '95,98' } else { '98,99' })
    $set = [ordered]@{
        # The process at high priority (never realtime) and a soft hold on
        # 90% of the memory: Windows takes the game's pages last.
        'VICTORY_PRIORITY' = 'high'
        'VICTORY_WORKING_SET_PERCENT' = '90'
        # Memory pressure: mild at 2%, critical at 1% of the memory (and of
        # what Windows can commit); the card at 98 and 99% of its budget.
        'VICTORY_PRESSURE_RAM_MB' = "$(2 * $ramCrit),$ramCrit"
        'VICTORY_PRESSURE_COMMIT_MB' = "$(2 * $commitCrit),$commitCrit"
        'VICTORY_PRESSURE_CARD_PERCENT' = $cardPressure
        # Textures up to 99% of the card's budget (90), up to 512 MB of
        # uploads a frame (192), 256 MB of kept staging (128).
        'VICTORY_TEXTURE_ROOM_PERCENT' = $texRoom
        'VICTORY_TEXTURE_STAGE_MB' = '512'
        'VICTORY_STAGING_POOL_MB' = '256'
        # 256 MB of compressed reads in flight (128; cool_fox / CGBJ).
        'VICTORY_DSTORAGE_STAGING_MB' = '256'
        # More of the game's buffers compared on the card (512 MB on cards
        # up to 14 GB): a fifth of the card, 1 GB at least.
        'VICTORY_CARD_BUFFERS_MB' = "$cardMb"
        # Pages compared on the CPU instead of waiting for the card (64 MB).
        'VICTORY_MERGE_UNWATCHED_MB' = '256'
        # Every thread but one: copy-back (8 at most before), texture
        # detiling (half before), pipeline compiles (two thirds before).
        'VICTORY_WIDE_THREADS' = "$use"
        'VICTORY_DETILE_THREADS' = "$([Math]::Min(16, $use))"
        'VICTORY_PIPELINE_WORKERS' = "$use"
    }
    foreach ($k in $set.Keys) { if ($null -eq $script:UserAll[$k]) { [Environment]::SetEnvironmentVariable($k, [string]$set[$k], 'Process') } }
    # No cap on textures (the low preset's 1536 MB).
    if ($null -eq $script:UserAll['VICTORY_TEXTURE_BUDGET_MB']) { [Environment]::SetEnvironmentVariable('VICTORY_TEXTURE_BUDGET_MB', $null, 'Process') }
    # 6.13: memory windows through the BAR up to 3072 MB or 35% of the card
    # (20-30% normally), where they are on at all (under 24 GB of memory).
    # 6.20: not on cards under 8 GB (see $small above): 20% as normally.
    if ($script:BarWindowsMb -gt 0 -and $null -eq $script:UserAll['VICTORY_WINDOWS_BAR_MB'] -and $vram -gt 0 -and -not $small) {
        $env:VICTORY_WINDOWS_BAR_MB = [string]([Math]::Min(3072, [int]($vram * 0.35)))
    }
    Say "ALL-OUT: this PC has $threads threads, $ramMb MB of memory ($virtMb MB with the page file), a $vram MB card; the game gets $use threads at high priority, textures up to $texRoom% of the card, no texture cap, memory guard at $ramCrit MB free   (MAX switches)"
    Bad '         all-out runs at the edge of this PC: if it stutters hard, pages to disk or loses the card, press MAX to go back.'
}
function Undo-AllOut {
    foreach ($k in @('VICTORY_PRIORITY', 'VICTORY_WORKING_SET_PERCENT', 'VICTORY_PRESSURE_RAM_MB', 'VICTORY_PRESSURE_COMMIT_MB', 'VICTORY_PRESSURE_CARD_PERCENT', 'VICTORY_TEXTURE_ROOM_PERCENT', 'VICTORY_TEXTURE_STAGE_MB', 'VICTORY_STAGING_POOL_MB', 'VICTORY_DSTORAGE_STAGING_MB', 'VICTORY_CARD_BUFFERS_MB', 'VICTORY_MERGE_UNWATCHED_MB', 'VICTORY_WIDE_THREADS', 'VICTORY_DETILE_THREADS', 'VICTORY_PIPELINE_WORKERS')) {
        if ($null -eq $script:UserAll[$k]) { [Environment]::SetEnvironmentVariable($k, $null, 'Process') }
    }
}

function Set-RunEnv {
    $c = $script:Config
    # 6.1: the game log's first line names this version (victory.exe prints it).
    $env:VICTORY_TRY_VERSION = $TryVersion
    $env:VICTORY_GAME_DIR = $c.game_dir
    $env:RUST_BACKTRACE = '1'
    # Long shader loops pass the 2 s driver timeout; 256 turns was the
    # stable middle on an 8 GB RTX 3060 Ti (victory-gta6 #18): 32 makes hair
    # glow and the scene darker, 1024 lost the device after 7 min.
    # 128 once a relay run has lost the card (see Check-Run).
    # (Whether the user set it before VIctory Try started, not a run before.)
    $script:LoopFromTry = -not $script:UserLoop
    if ($script:LoopFromTry) { $env:VICTORY_SHADER_LOOP_LIMIT = $(if ($c.loop) { $c.loop } else { '256' }) }
    # Speed: results land as the card finishes (2-3 fps and stable on that
    # card). F mode instead sends submissions ahead with card buffers: 4-5 fps
    # on a 12 GB RTX 4070 (victory-gta6 #27), lost the device sooner on an
    # 8 GB laptop.
    $fastVars = @(@('VICTORY_LATE_WAIT', '3'), @('VICTORY_WIDE_BACK', '1'), @('VICTORY_CARD_BUFFERS_MIN_MB', '1'), @('VICTORY_CARD_BUFFERS_MB', '1024'))
    if ($c.fast -eq '1') {
        foreach ($kv in $fastVars) { [Environment]::SetEnvironmentVariable($kv[0], $kv[1], 'Process') }
        # (3.7: the relay is on by default now; F mode keeps it off as before.)
        [Environment]::SetEnvironmentVariable('VICTORY_RELAY', '0', 'Process')
        $script:WideFromTry = $false
        Say 'speed: F mode (late waits, card buffers)   (F switches back)'
    } else {
        foreach ($kv in $fastVars) { [Environment]::SetEnvironmentVariable($kv[0], $null, 'Process') }
        [Environment]::SetEnvironmentVariable('VICTORY_RELAY', '2', 'Process')
        # 3.5: what the card wrote is found on several threads (F mode's
        # VICTORY_WIDE_BACK) instead of one: the world's ~200 ms a frame of
        # copying back was one core's memory speed. Off again by itself if a
        # run loses the card (see Check-Run).
        $script:WideFromTry = $c.wide -ne 'off'
        # (3.7: on by default now, so off has to be said.)
        [Environment]::SetEnvironmentVariable('VICTORY_WIDE_BACK', $(if ($script:WideFromTry) { '1' } else { '0' }), 'Process')
        Say "speed: relay (stable)$(if ($script:WideFromTry) { ', copy-back on several threads' })   (F switches to the faster, riskier mode)"
        # 6.0: the card looks for what shaders wrote in title buffers of
        # 4 MB and more (16 MB and more on 6-12 GB cards before): in the
        # 2026-11-26 report the CPU compared 100-150 MB a frame, 70-99 MB of
        # it in buffers of 1 MB or more, while the card stood 40-74% idle.
        # 6.7: 2 MB (4 before): the 6.6 report still compared 87.7 MB a
        # frame on the CPU, 77.6 MB of it in copies of 1 MB or more.
        if ($null -eq $script:UserFx['VICTORY_CARD_BUFFERS_MIN_MB']) { $env:VICTORY_CARD_BUFFERS_MIN_MB = '2' }
    }
    # 3.6: compute shaders that need 64-lane waves (refused by 32-lane
    # NVIDIA cards) are translated a slower way that works. Off again by
    # itself if a run loses the card (see Check-Run).
    $script:WaveWideFromTry = (-not $script:UserWaveWide) -and $c.wavewide -ne 'off'
    if (-not $script:UserWaveWide) { [Environment]::SetEnvironmentVariable('VICTORY_WAVE_WIDE', $(if ($c.wavewide -eq 'off') { '0' } else { $null }), 'Process') }
    # 3.8: stand-in pipelines, upload memory on the card (Resizable BAR) and
    # buffers compared on the card from the PC's memory. All three off again
    # by themselves if a run loses the card (see Check-Run).
    $script:NewFromTry = $c.new38 -ne 'off'
    # 3.9: lazy copy-back (the title hears the card is done before what it
    # wrote is copied back). Off again by itself after a run that crashed
    # with an access violation (see Check-Run).
    $script:LazyFromTry = $c.lazy -ne 'off'
    if (-not $script:LazyFromTry) { [Environment]::SetEnvironmentVariable('VICTORY_LAZY_BACK', '0', 'Process') }
    # 6.14: a lost card long ago had turned the 3.8 set off for good on some
    # PCs, and with it two things that are not risky and that the frame rate
    # needs most: the fetches of what shaders wrote on the card's copy engine
    # (VICTORY_COPY_FETCH) and barrier trimming (VICTORY_TRIM_BARRIERS). The
    # 6.12 report (RTX 4060, all-out on): the thread that copies back waited
    # 133 ms of every 220 ms frame for those fetches, queued behind every
    # flight on the card, and 0 barriers were trimmed: 4.5 fps. Both are on
    # again, each on its own switch now (the stand-in pipelines and PC-side
    # card copies stay with the 3.8 switch), and they are the last things a
    # lost card turns off (see Check-Run).
    if ($c.mig613 -ne '1') {
        if ($c.new38 -eq 'off') { Say '6.14: the copy-engine fetches and barrier trimming are back on (a lost card long ago had turned them off; that cost about 130 ms a frame in the 6.12 report).' }
        # Lazy copy-back too, one more time: a crash long ago had turned it
        # off, and without it the game hears the card is done only after
        # the whole copy-back (the console never copies back at all). Off
        # again by itself if a run crashes on a memory access (see Check-Run).
        if ($c.lazy -eq 'off') { Say '6.14: lazy copy-back is on again for one more try (the game no longer waits for the copy-back to hear a frame is done).' }
        $c.lazy = ''; $c.mig613 = '1'
        $script:LazyFromTry = $true
        [Environment]::SetEnvironmentVariable('VICTORY_LAZY_BACK', $null, 'Process')
        Save-Config
    }
    if (-not $script:NewFromTry) {
        foreach ($k in @('VICTORY_PC_CARD_MB')) { [Environment]::SetEnvironmentVariable($k, '0', 'Process') }
    }
    # 6.15: new things on screen at once. Stand-in pipelines on their own
    # switch (config standin), no longer tied to the 3.8 set an old lost card
    # had turned off (the 6.12 report ran with VICTORY_STAND_IN=0: every draw
    # of a new object whose pipeline was still compiling was dropped, 19-24 a
    # frame when the world streamed). A stand-in is a finished pipeline of
    # the same shaders that differs only in culling, depth test or blend
    # factors: the object is drawn this frame. VICTORY_STAND_IN_FIRST (on in
    # the engine, Kelvaroq 0050's idea) takes it before any wait, so the
    # frame does not stand still for the compile either. 6.22: no wait for
    # pipelines with no stand-in either (0 ms a frame; 250 before, 500
    # before that), as ScarlettPort 0.5.2 after player reports: on a first
    # run new pipelines come every frame, and each frame stood up to the
    # whole wait. Their draws are left out until the pipeline is made, a
    # frame or a few; a dispatch still waits for its compute pipeline
    # (VICTORY_PIPELINE_COMPUTE_WAIT). VICTORY_PIPELINE_WAIT_MS=250 waits as
    # before. Off again by itself if a run loses the card (see Check-Run).
    $script:StandInFromTry = $c.standin -ne 'off'
    if ($null -eq $script:UserAll['VICTORY_STAND_IN']) { [Environment]::SetEnvironmentVariable('VICTORY_STAND_IN', $(if ($script:StandInFromTry) { $null } else { '0' }), 'Process') }
    if ($null -eq $script:UserAll['VICTORY_PIPELINE_WAIT_MS']) { [Environment]::SetEnvironmentVariable('VICTORY_PIPELINE_WAIT_MS', '0', 'Process') }
    # 6.14: the card's hashes of copies' pieces by a group of 64 threads a
    # piece instead of one (VICTORY_CARD_HASH_WIDE=1; fork #1014 by
    # martindevbr, made the default in Kelvaroq 0041 by stellarr-studio:
    # heavy frames 198 -> 120 ms there). With one thread a piece, each of the
    # 105 hash passes and 58 compares a frame of the 6.12 report took the time
    # of 4,096 loads one after the other. Only the card's own hashes change.
    $script:WideHashFromTry = ($null -eq $script:UserAll['VICTORY_CARD_HASH_WIDE']) -and $c.widehash -ne 'off'
    if ($null -eq $script:UserAll['VICTORY_CARD_HASH_WIDE']) { [Environment]::SetEnvironmentVariable('VICTORY_CARD_HASH_WIDE', $(if ($script:WideHashFromTry) { '1' } else { $null }), 'Process') }
    # 6.14: one frame ahead, as a console runs a game. On the Series S the
    # game's render thread records frame N+1 while the GPU draws frame N, and
    # its present returns at once. Here the game's thread prepared every
    # submission itself (copies, write watch, descriptors: 50-55 ms a frame in
    # the 6.12 report) and then stood in the present until all of it was
    # sent. VICTORY_ASYNC_SUBMIT=1: a thread of ours prepares and sends the
    # submissions in order; VICTORY_ASYNC_PRESENT=1: the present goes behind
    # them and the game records the next frame meanwhile (one frame ahead at
    # most). Both on by default in Kelvaroq (stellarr-studio, 0040/0048:
    # present p95 568 -> 262 ms, heavy frame 115 -> 108 ms). Off again by
    # itself if a run loses the card (see Check-Run).
    $script:AheadFromTry = $c.ahead -ne 'off' -and $c.fast -ne '1'
    foreach ($k in @('VICTORY_ASYNC_SUBMIT', 'VICTORY_ASYNC_PRESENT')) {
        if ($null -eq $script:UserAll[$k]) { [Environment]::SetEnvironmentVariable($k, $(if ($script:AheadFromTry) { '1' } else { $null }), 'Process') }
    }
    # 6.17: the pass over the game's descriptor heap (the records it copies
    # into its view heap itself, 25,000-30,000 new ones a frame in the 6.15
    # report: 22-39 ms a frame) on a thread of its own, beside the preparing
    # of the submission, instead of on the submitting thread before it. The
    # card stood idle 68-112 ms a frame behind that thread. The engine has
    # had this as VICTORY_SYNC_BESIDE (off unless set); done before any list
    # of the submission goes to the card. Off again by itself if a run loses
    # the card (see Check-Run); config.json beside=off.
    $script:BesideFromTry = ($null -eq $script:UserAll['VICTORY_SYNC_BESIDE']) -and $c.beside -ne 'off'
    if ($null -eq $script:UserAll['VICTORY_SYNC_BESIDE']) { [Environment]::SetEnvironmentVariable('VICTORY_SYNC_BESIDE', $(if ($script:BesideFromTry) { '1' } else { $null }), 'Process') }
    Say "frame rate: the descriptor heap pass beside the preparing (6.17) $(if ($env:VICTORY_SYNC_BESIDE -eq '1') { 'ON' } else { 'off' })"
    $script:CopyFetchFromTry = $c.copyfetch -ne 'off'
    $script:TrimFromTry = $c.trim -ne 'off'
    foreach ($kv in @(@('VICTORY_COPY_FETCH', $script:CopyFetchFromTry), @('VICTORY_TRIM_BARRIERS', $script:TrimFromTry))) {
        if ($null -eq $script:UserAll[$kv[0]]) { [Environment]::SetEnvironmentVariable($kv[0], $(if ($kv[1]) { $null } else { '0' }), 'Process') }
    }
    Say "frame rate: card results fetched on the copy engine $(if ($env:VICTORY_COPY_FETCH -eq '0') { 'off' } else { 'ON' }), barrier trimming $(if ($env:VICTORY_TRIM_BARRIERS -eq '0') { 'off' } else { 'ON' }), card hashes by 64 threads a piece $(if ($env:VICTORY_CARD_HASH_WIDE -eq '1') { 'ON' } else { 'off' }), one frame ahead (console-style) $(if ($env:VICTORY_ASYNC_SUBMIT -eq '1') { 'ON' } else { 'off' }), lazy copy-back $(if ($env:VICTORY_LAZY_BACK -eq '0') { 'off' } else { 'ON' }), stand-ins for new objects $(if ($env:VICTORY_STAND_IN -eq '0') { 'off' } else { 'ON' })"
    # 6.4: upload memory on the card (Resizable BAR) on its own switch, on
    # unless a lost card turned it off (it was off with the other 3.8
    # additions since a lost card long ago). The CPU then writes texture
    # staging straight into the card's memory while the card has room.
    $script:RebarFromTry = $c.rebar -ne 'off'
    [Environment]::SetEnvironmentVariable('VICTORY_GPU_UPLOAD', $(if ($script:RebarFromTry) { $null } else { '0' }), 'Process')
    # 6.13: on PCs with less than 24 GB of memory, the game's memory windows
    # go straight into the card's memory through the BAR (one copy, none in
    # the PC's memory), from trustno1-coder/VIctory-NVIDIA (Indy2l): on a
    # 16 GB RTX 3060 PC the out-of-memory 0x8007000E and the driver fault a
    # minute in went away, shared memory stayed at 4.4 GB instead of 7.5 and
    # fps went from 1.6-3 to 10-12. Up to 3072 MB, and at most 30% of the
    # card here (20% on cards under 8 GB: about 1.2 GB of a 6 GB card, whose
    # textures and screens already filled it in the 6.7 report), so textures
    # keep their room; past 80% of the card no window goes there anyway.
    # Nothing changes without Resizable BAR; off with it after a lost card.
    $script:BarWindowsMb = 0
    if ($script:RebarFromTry -and $c.barwin -ne 'off') {
        $ramNow = [int64](Ram-Info)[0]
        $vramNow = $(if ($c.vram) { [int]$c.vram } else { Card-Vram })
        if ($ramNow -gt 0 -and $ramNow -lt 24576) {
            $share = $(if ($vramNow -gt 0 -and $vramNow -lt 8000) { 0.20 } else { 0.30 })
            $script:BarWindowsMb = $(if ($vramNow -gt 0) { [Math]::Min(3072, [int]($vramNow * $share)) } else { 1024 })
        }
    }
    if ($null -eq $script:UserAll['VICTORY_WINDOWS_BAR_MB']) {
        [Environment]::SetEnvironmentVariable('VICTORY_WINDOWS_BAR_MB', $(if ($script:BarWindowsMb -gt 0) { [string]$script:BarWindowsMb } else { $null }), 'Process')
    }
    Say "memory windows on the card through Resizable BAR: $(if ($env:VICTORY_WINDOWS_BAR_MB) { "up to $($env:VICTORY_WINDOWS_BAR_MB) MB (used only if the card offers it; the log says 'through a resizable BAR')" } else { 'off (24 GB or more of memory, or BAR off after a lost card)' })"
    # 6.6: with the relay, what the card's compares flag comes down to the PC
    # inside the same flight, not with a list of its own queued behind newer
    # flights (the 6.5 report: 124-156 ms a frame of that thread waiting for
    # such fetches). Off with the other new additions after a lost card.
    # 6.7: on a switch of its own. 6.6 tied it to the 3.8 additions, which a
    # lost card long ago had turned off on some PCs: the 6.6 report shows it
    # never ran there (no "inside their flights" line). Off again by itself
    # if a run with it loses the card (see Check-Run).
    # 6.8: off by default. On a 6 GB RTX 4050 the card's time per frame
    # went up about threefold with it (the 6.7 report), as on an RTX 5060 Ti
    # (Linox-IDK: about 100 -> 600 ms). VICTORY_CARD_FETCH_INFLIGHT=1 still
    # turns it on.
    $script:InflightFromTry = ($null -eq $script:UserAll['VICTORY_CARD_FETCH_INFLIGHT']) -and $c.inflight -eq 'on' -and $c.fast -ne '1'
    if ($null -eq $script:UserAll['VICTORY_CARD_FETCH_INFLIGHT']) {
        [Environment]::SetEnvironmentVariable('VICTORY_CARD_FETCH_INFLIGHT', $(if ($script:InflightFromTry) { '1' } else { $null }), 'Process')
    }
    # 6.7: more lists fly before the submitting thread waits for the card
    # (384, VICTORY's own 256): in the 6.6 report that wait was 75-100 ms a
    # frame ("lists in the air") while the card stood idle 65.9 ms a frame
    # behind that thread. VICTORY goes back to 256 by itself if the
    # descriptor slots the lists hold run out, and the next run starts at
    # 256 (see Check-Run).
    if ($null -eq $script:UserAll['VICTORY_LATE_WAIT_LISTS']) {
        [Environment]::SetEnvironmentVariable('VICTORY_LATE_WAIT_LISTS', $(if ($c.slots -eq 'out' -or $c.fast -eq '1') { $null } else { '384' }), 'Process')
    }
    # 6.7: the upscaler's stand-in accumulates frames (a temporal resolve,
    # as the console's upscaler does) instead of one plain stretch: hair is
    # no longer dotted or see-through, and soft shadows stop crawling. T
    # switches it.
    if ($null -eq $script:UserAll['VICTORY_UPSCALE_TEMPORAL']) {
        [Environment]::SetEnvironmentVariable('VICTORY_UPSCALE_TEMPORAL', $(if ($c.temporal -eq 'off') { '0' } else { '1' }), 'Process')
    }
    # 6.10: AMD's PC FidelityFX upscaler (FSR 3.1 on NVIDIA, FSR 4 on new
    # Radeons) makes the game's upscale instead of our stand-in, as on
    # Belobug's engine (PortWatch #1, ported from DexNom): the console's
    # upscaler is FidelityFX too, so the picture is what the game was made
    # for (no blocks on lit and reflecting surfaces). T switches.
    if ($null -eq $script:UserAll['VICTORY_FIDELITYFX_DIR']) {
        $fsrDir = $(if ($c.temporal -ne 'own' -and $c.temporal -ne 'off' -and $c.plain -ne '1') { Ensure-Fsr } else { $null })
        [Environment]::SetEnvironmentVariable('VICTORY_FIDELITYFX_DIR', $fsrDir, 'Process')
    }
    $upName = $(if ($env:VICTORY_FIDELITYFX_DIR) { 'AMD FidelityFX (PC)' } elseif ($env:VICTORY_UPSCALE_TEMPORAL -ne '0') { 'own (frames accumulated)' } else { 'plain stretch' })
    Say "picture: upscaler $upName   (T switches FSR / own / off)"
    Say "speed: card results fetched inside their flights $(if ($env:VICTORY_CARD_FETCH_INFLIGHT -eq '1') { 'ON' } else { 'off' }), lists in the air before a wait: $(if ($env:VICTORY_LATE_WAIT_LISTS) { $env:VICTORY_LATE_WAIT_LISTS } else { '256' })"
    # 4.1: the screen the console says it is plugged into. 1080p by default
    # (a Series X on a 1080p TV): at 4K the game renders 2560x1440 inside and
    # upscales to 3840x2160 every frame, on an 8 GB card that is also
    # emulating. R switches to 4K. (Not when VICTORY_DISPLAY_MODE was set
    # before VIctory Try started.)
    if (-not $script:UserDisplay) { $env:VICTORY_DISPLAY_MODE = $(if ($c.res -eq '4k') { '3840x2160' } else { '1920x1080' }) }
    Say "screen: $($env:VICTORY_DISPLAY_MODE)   (R switches 1080p / 4K)"
    # The hit-detection program that page-faults the card (upstream #981),
    # skipped as the NVIDIA world runs on victory-gta6 did.
    # And the meshlet culling that hung at load (b83629b90cd2b7bc, #18 there).
    if (-not $env:VICTORY_SKIP_PIPELINES) { $env:VICTORY_SKIP_PIPELINES = 'eeed82b877b44b5f,b83629b90cd2b7bc' }
    # 4.1: plain mode (Z): every speed and safety switch above back at
    # VICTORY's own defaults, as ztrx's launcher (scripts\play.ps1 there)
    # runs the game; only the paths, cache folders, screen size and the two
    # skipped programs stay. The lost-card steps then have nothing to undo.
    if ($c.plain -eq '1') {
        foreach ($k in @('VICTORY_SHADER_LOOP_LIMIT', 'VICTORY_LATE_WAIT', 'VICTORY_WIDE_BACK', 'VICTORY_CARD_BUFFERS_MIN_MB', 'VICTORY_CARD_BUFFERS_MB', 'VICTORY_RELAY', 'VICTORY_WAVE_WIDE', 'VICTORY_LAZY_BACK', 'VICTORY_STAND_IN', 'VICTORY_GPU_UPLOAD', 'VICTORY_PC_CARD_MB', 'VICTORY_COPY_FETCH', 'VICTORY_TRIM_BARRIERS', 'VICTORY_CARD_FETCH_INFLIGHT', 'VICTORY_LATE_WAIT_LISTS', 'VICTORY_CARD_HASH_WIDE', 'VICTORY_ASYNC_SUBMIT', 'VICTORY_ASYNC_PRESENT', 'VICTORY_PIPELINE_WAIT_MS')) {
            [Environment]::SetEnvironmentVariable($k, $null, 'Process')
        }
        $script:LoopFromTry = $false; $script:WideFromTry = $false; $script:WaveWideFromTry = $false
        $script:NewFromTry = $false; $script:LazyFromTry = $false; $script:RebarFromTry = $false; $script:InflightFromTry = $false; $script:WideHashFromTry = $false; $script:AheadFromTry = $false; $script:CopyFetchFromTry = $false; $script:TrimFromTry = $false; $script:StandInFromTry = $false
        Say 'PLAIN mode: VICTORY''s defaults, as ztrx runs the game   (Z switches back to Try''s settings)'
    }
    # 4.2: the frame-rate ideas. B (on by default): the game renders at 75%
    # of the screen and our present scales it up and sharpens it (as DLSS /
    # FSR mods do), and ray tracing is skipped (its structures are not built
    # here: the rays only cost time and gave black shadows). M: frames moved
    # on between the game's own (as VR's spacewarp). H: small buffers shared
    # with the card instead of copied (as on one-memory machines).
    # (Each only when not set before VIctory Try started.)
    foreach ($kv in @(@('VICTORY_RENDER_SCALE', '75'), @('VICTORY_SKIP_RT', '1'), @('VICTORY_FRAMEGEN', '1'), @('VICTORY_INPLACE', '3'))) {
        $name = $kv[0]
        if ($null -eq $script:UserFx[$name]) {
            $want = switch ($name) {
                'VICTORY_FRAMEGEN' { $c.between -eq '1' -and $c.console -ne 'series-s' }
                'VICTORY_INPLACE' { $c.hybrid -eq '1' }
                # (Series S renders smaller and upscales by itself: 4.6)
                'VICTORY_RENDER_SCALE' { $c.boost -ne 'off' -and $c.console -ne 'series-s' }
                default { $c.boost -ne 'off' }
            }
            [Environment]::SetEnvironmentVariable($name, $(if ($want) { $kv[1] } else { $null }), 'Process')
        }
    }
    # 6.22: every title buffer of 64 KiB and more gets a copy in the card's
    # memory (VICTORY_CARD_BUFFERS_MIN_KB=64, as ScarlettPort's Stable
    # preset): the small ones are nearly all there are and few bytes, and in
    # the PC's memory every load and store a shader makes on them crosses the
    # bus (fxceless measured, RTX 3060 12 GB: the title's lists 68.0 -> 49.8
    # ms of card time a frame). The copies stay inside the card buffer cap.
    # Not with the hybrid (H: small buffers are the title's own memory
    # there), not in plain mode, and not over a VICTORY_CARD_BUFFERS_MIN_MB or
    # _MIN_KB set before VIctory Try started (_MIN_KB=0 turns it off).
    if ($null -eq $script:UserAll['VICTORY_CARD_BUFFERS_MIN_KB']) {
        $minKb = ($null -eq $script:UserAll['VICTORY_CARD_BUFFERS_MIN_MB']) -and ($null -eq $script:UserAll['VICTORY_INPLACE']) -and $c.hybrid -ne '1' -and $c.plain -ne '1'
        [Environment]::SetEnvironmentVariable('VICTORY_CARD_BUFFERS_MIN_KB', $(if ($minKb) { '64' } else { $null }), 'Process')
    }
    # 4.4: the picture fixes. Ambient light (Belobug/VIctory-Belobug-AMD):
    # the game's light that bounces around comes from ray-traced probes that
    # do not run here, so whatever the sun did not reach was black; a soft
    # constant light replaces it, shaded in corners from the depth. A
    # switches it. Levels on demand (fxceless): a texture takes the card's
    # memory only for the sizes of it the game holds; the 8 GB card filled
    # at about 4.9 GB of textures and 16,000 more waited for good (doors,
    # props and people that came in later stayed black or missing). L
    # switches it. And the pool-full stop about 8 minutes in (mangrove,
    # VIctory #698): the allocation returns nothing instead. (Each only when
    # not set before VIctory Try started.)
    # 6.4: 0.45 by default (0.6 made people too bright); A goes normal ->
    # low (0.3) -> high (0.6, as before 6.4) -> off.
    if ($null -eq $script:UserFx['VICTORY_AMBIENT']) { $env:VICTORY_AMBIENT = $(switch ($c.ambient) { 'off' { 'off' } 'low' { '0.3' } 'high' { '0.6' } default { '0.45' } }) }
    if ($null -eq $script:UserFx['VICTORY_LEVELS_ON_DEMAND']) { $env:VICTORY_LEVELS_ON_DEMAND = $(if ($c.levels -eq 'off') { '0' } else { '1' }) }
    if ($null -eq $script:UserFx['VICTORY_POOL_NO_ASSERT']) { $env:VICTORY_POOL_NO_ASSERT = '1' }
    # 4.5: walls, ceilings and floors. The shader clean-up passes left the
    # prison cell's walls, roof and floor out (sky where they should be) on an
    # RTX 4080 and an RX 7800 XT, and VICTORY_SHADER_OPT=0 brought them back
    # (Xeltax/VIcky, same save A/B): off by default now, O switches. The 14
    # cascade-shadow programs are wrong in one 64-lane form on 32-lane NVIDIA
    # cards (missing ceiling, dark room: fxceless/victory-fork #62): the
    # translator keeps them out of it by itself now; W switches that off
    # (VICTORY_WAVE_DEEP_SKIPS_NOT=none) for a comparison.
    # 6.16: the card's time measured by kind of list and by the game's passes
    # (two timestamps a list), so a report says what the card spends its
    # frame on: in the 6.15 report it was busy about 200 ms of every 300-420.
    # VICTORY_CARD_METER=0 (or meter=off in config.json): not measured.
    if ($null -eq $script:UserAll['VICTORY_CARD_METER']) { $env:VICTORY_CARD_METER = $(if ($c.meter -eq 'off') { '0' } else { '1' }) }
    if ($null -eq $script:UserFx['VICTORY_SHADER_OPT']) { [Environment]::SetEnvironmentVariable('VICTORY_SHADER_OPT', $(if ($c.opt -eq 'on') { $null } else { '0' }), 'Process') }
    if ($null -eq $script:UserFx['VICTORY_WAVE_DEEP_SKIPS_NOT']) { [Environment]::SetEnvironmentVariable('VICTORY_WAVE_DEEP_SKIPS_NOT', $(if ($c.shadowfix -eq 'off') { 'none' } else { $null }), 'Process') }
    # 4.7: speed. The game's own buffer-to-buffer copies run on the card
    # (pablogonz12/Scarlight #86): on the CPU each one that touched memory
    # the card was still writing made us wait for the card to finish
    # everything first, about 157 ms of every 485 ms frame in the 2026-11-25
    # report. And the card's checks of what we sent up are done in one pass
    # per run of pieces instead of one per range (about 1,900 tiny passes a
    # frame). K switches both. (Each only when not set before VIctory Try
    # started.)
    if ($null -eq $script:UserFx['VICTORY_GPU_COPIES']) { $env:VICTORY_GPU_COPIES = $(if ($c.gpucopy -eq 'off') { '0' } else { '1' }) }
    # 4.8: two of AvatarManz's run settings (avatarmanz/VIctory-7900XTX's
    # play.ps1) this fork has too: indirect mesh draws clamped a window of
    # records at a time, and title copies that continue each other joined.
    if ($null -eq $script:UserFx['VICTORY_INDIRECT_WINDOW']) { $env:VICTORY_INDIRECT_WINDOW = '1' }
    if ($null -eq $script:UserFx['VICTORY_JOIN_COPIES']) { $env:VICTORY_JOIN_COPIES = '1' }
    # 4.9: the card is kept busy between the title's submissions so its clocks
    # stay up (avatarmanz's VICTORY_CARD_AWAKE): in 4.8 logs the card stood
    # idle 180 ms a frame behind the submitting thread.
    if ($null -eq $script:UserFx['VICTORY_CARD_AWAKE']) { $env:VICTORY_CARD_AWAKE = '1' }
    if ($null -eq $script:UserFx['VICTORY_CARD_REHASH_RUNS']) { [Environment]::SetEnvironmentVariable('VICTORY_CARD_REHASH_RUNS', $(if ($c.gpucopy -eq 'off') { $null } else { '1' }), 'Process') }
    # 6.8: fxceless's speed set (fxceless/victory-fork, merged whole in
    # 6.8; 8.2-8.6 world fps on an RTX 4070 there). The heap pass runs on a
    # thread of its own beside the submitting one (VICTORY_HEAP_WORKER: it
    # was 62-115 ms a frame, 92 ms in the 6.7 report), texture checks run
    # side by side with the preparing (VICTORY_PARALLEL_REFRESH), settles
    # make room in one go (VICTORY_SETTLE_ROOM), found copies are looked up
    # once per table (VICTORY_LOOKUP_CACHE), the small textures the water
    # reads come back from the card (VICTORY_TEXTURE_BACK) and 512 MB of
    # buffer windows live on the card (VICTORY_CARD_WINDOWS_MB: no black
    # textures). Y switches the set; off by itself if a run loses the card.
    $script:FxFromTry = ($c.fxspeed -ne 'off') -and ($c.plain -ne '1')
    # 6.11: and parts that follow one another in the queues go to the card as
    # one submission, prepared once (VICTORY_JOIN_PARTS: viktoschi's tip; on
    # in lepotane/VIctory-leonida's launcher, 19.4 fps on an RTX 4060 Ti).
    foreach ($kv in @(@('VICTORY_HEAP_WORKER', '1'), @('VICTORY_PARALLEL_REFRESH', '1'), @('VICTORY_SETTLE_ROOM', '1'), @('VICTORY_LOOKUP_CACHE', '1'), @('VICTORY_TEXTURE_BACK', '1'), @('VICTORY_CARD_WINDOWS_MB', '512'), @('VICTORY_JOIN_PARTS', '1'))) {
        if ($null -eq $script:UserAll[$kv[0]]) { [Environment]::SetEnvironmentVariable($kv[0], $(if ($script:FxFromTry) { $kv[1] } else { $null }), 'Process') }
    }
    # 6.8: the low preset (X) for 6 GB cards (on by itself under 7000 MB):
    # the game is told a 1280x720 screen (less to scale up and post-process
    # every frame; the window still fills the screen), and texture copies
    # stop at 1536 MB of the card (sizes past that wait: blurrier, but the
    # card no longer runs out: "Critical" all through the 6.7 report).
    $lowAuto = ($c.vram -and [int]$c.vram -lt 7000)
    $script:LowFromTry = ($c.low -eq '1' -or ($c.low -ne 'off' -and $lowAuto)) -and ($c.plain -ne '1')
    if ($script:LowFromTry) {
        if (-not $script:UserDisplay) { $env:VICTORY_DISPLAY_MODE = '1280x720' }
        if ($null -eq $script:UserAll['VICTORY_TEXTURE_BUDGET_MB']) { $env:VICTORY_TEXTURE_BUDGET_MB = '1536' }
    } elseif ($null -eq $script:UserAll['VICTORY_TEXTURE_BUDGET_MB']) {
        [Environment]::SetEnvironmentVariable('VICTORY_TEXTURE_BUDGET_MB', $null, 'Process')
    }
    Say "speed: fxceless's set (heap thread, side-by-side texture checks, settle room, lookup cache, joined submissions) $(if ($script:FxFromTry) { 'ON' } else { 'off' })   (Y switches)"
    Say "low preset: $(if ($script:LowFromTry) { "ON (game screen $($env:VICTORY_DISPLAY_MODE), textures up to $($env:VICTORY_TEXTURE_BUDGET_MB) MB)" } else { 'off' })   (X switches)"
    Say "speed: copies on the card $(if ($env:VICTORY_GPU_COPIES -ne '0') { 'ON' } else { 'off' }), checks joined $(if ($env:VICTORY_CARD_REHASH_RUNS -eq '1') { 'ON' } else { 'off' })   (K switches)"
    Say "walls: shader clean-up $(if ($env:VICTORY_SHADER_OPT -eq '0') { 'off (walls and roofs kept)' } else { 'ON' })   (O switches)   shadow fix $(if ($env:VICTORY_WAVE_DEEP_SKIPS_NOT -eq 'none') { 'off' } else { 'ON' })   (W switches)"
    Say "picture: ambient light $(if ($env:VICTORY_AMBIENT -ne 'off') { $env:VICTORY_AMBIENT } else { 'off' })   (A: normal 0.45 / low 0.3 / high 0.6 / off)   textures level by level $(if ($env:VICTORY_LEVELS_ON_DEMAND -eq '1') { 'ON' } else { 'off' })   (L switches)"
    Say "fps boost: $(if ($c.boost -ne 'off') { 'ON (75% render scale + sharpen, no ray tracing)' } else { 'off' })   frames in between: $(if ($c.between -eq '1') { 'ON' } else { 'off' })   shared small buffers: $(if ($c.hybrid -eq '1') { 'ON' } else { 'off' })"
    # 6.12: the ray-tracing passes that turned every missed ray into light
    # (white, hazy rooms and reflections with the view angle) are not built
    # or run while ray tracing is skipped (B): trustno1-coder/VIctory-NVIDIA's
    # list (1,000 CS_RaytracingResolve and 34 traced-shadow shaders). Fewer
    # pipelines to compile, too. And a colour a shader writes that is NaN or
    # infinite becomes 0 (VICTORY_NAN_SCRUB, from the same fork): one bad
    # value no longer spreads into black blocks on faces and blocky shine
    # where light reflects through the blur and the upscaler.
    $rtList = Join-Path $Src 'scripts\try\rt-off-hashes.txt'
    # 6.21: handed over as a file (VICTORY_DENY_PIPELINES_FILE, after
    # fxceless/victory-fork f9e0371) instead of 17 KB of environment.
    if ($null -eq $script:UserAll['VICTORY_DENY_PIPELINES'] -and $null -eq $script:UserAll['VICTORY_DENY_PIPELINES_FILE']) {
        $useList = ($c.boost -ne 'off' -and $c.plain -ne '1' -and (Test-Path -LiteralPath $rtList))
        [Environment]::SetEnvironmentVariable('VICTORY_DENY_PIPELINES', $null, 'Process')
        [Environment]::SetEnvironmentVariable('VICTORY_DENY_PIPELINES_FILE', $(if ($useList) { $rtList } else { $null }), 'Process')
    }
    if ($null -eq $script:UserAll['VICTORY_NAN_SCRUB']) { [Environment]::SetEnvironmentVariable('VICTORY_NAN_SCRUB', $(if ($c.plain -ne '1') { '1' } else { $null }), 'Process') }
    $denied = $(if ($env:VICTORY_DENY_PIPELINES) { @($env:VICTORY_DENY_PIPELINES -split ',').Count } else { 0 })
    if ($env:VICTORY_DENY_PIPELINES_FILE -and (Test-Path -LiteralPath $env:VICTORY_DENY_PIPELINES_FILE)) { $denied += @(Get-Content -LiteralPath $env:VICTORY_DENY_PIPELINES_FILE | Where-Object { $_ -match '^[0-9a-fA-F]{16}$' }).Count }
    Say "picture: ray-tracing light passes left out: $(if ($denied) { "$denied (B switches)" } else { 'none' }), NaN colours cleared $(if ($env:VICTORY_NAN_SCRUB -eq '1') { 'ON' } else { 'off' })"
    # 6.12: all-out mode last, so it lifts the caps set above.
    if (AllOut) { Set-AllOut } else { Undo-AllOut; Say 'all-out mode: off   (MAX gives the game everything this PC has but about 1%)' }
    # Its own cache: the old base's pipeline library and shaders do not mix.
    if (-not $env:VICTORY_CACHE_DIR -or $env:VICTORY_CACHE_DIR -eq (Join-Path $TryHome 'cache-8gb')) { $env:VICTORY_CACHE_DIR = Join-Path $TryHome 'cache-gta6' }
    # Shaders that do not translate are saved here (zip and send this folder).
    if (-not $env:VICTORY_DUMP_FAILED) { $env:VICTORY_DUMP_FAILED = Join-Path $TryHome 'failed-shaders' }
}

# The recipes runs keep of the pipelines the game asked for (see Prepare-Shaders).
function Recipes-File { return Join-Path $env:VICTORY_CACHE_DIR 'pipelines\recipes.bin' }
function Recipes-Size {
    $f = Recipes-File
    if (Test-Path -LiteralPath $f) { return [string](Get-Item -LiteralPath $f).Length }
    return '0'
}

# With the game closed (victory.exe prepare): compiles every pipeline the
# game asked for in earlier runs (the recipes each run keeps) into the
# pipeline library, translating their shaders on the way, on all cores but
# two. The next launch then has them all from its start instead of
# dropping their draws (see-through world) while it compiles them a few a
# minute: one run that reaches a place is enough, no more restarting to get
# further. Runs after each new build and after each run that asked for new
# pipelines; quick when there is nothing new.
# 6.11: once per build also every shader in the game's files is translated
# into the shader cache (as AvatarManz's "victory shaders" does): a pipeline
# the game meets for the first time in play then only needs the driver's
# compile, not our translation as well (the hitches while "shaders compile").
function Prepare-Shaders([string]$why) {
    $c = $script:Config
    Step "Preparing pipelines with the game closed ($why)"
    Say 'Compiles everything the game asked for, so the next launch shows it at once. Can take a few minutes the first time.'
    $log = Join-Path $LogDir 'prepare.log'
    $all = ("$($c.shaders_all)" -ne "$($c.built_key)") -and $c.game_dir -and (Test-Path -LiteralPath $c.game_dir) -and ($env:VICTORY_TRY_ALL_SHADERS -ne '0')
    if ($all) { Say 'Once per build: every shader in the game''s files is translated too (10-30 minutes the first time; stopping it is fine, the game translates the rest itself).' }
    Push-Location $TryHome
    try {
        if ($all) {
            & (Ours-Exe) prepare $c.game_dir 2>&1 | ForEach-Object { "$_" } | Tee-Object -FilePath $log | Where-Object { $_ -match '^(prepar|translat)' }
        } else {
            & (Ours-Exe) prepare 2>&1 | ForEach-Object { "$_" } | Tee-Object -FilePath $log | Where-Object { $_ -match '^(prepar|translat)' }
        }
        $code = $LASTEXITCODE
    } finally { Pop-Location }
    if ($code -eq 0) { Say 'pipelines prepared' }
    else { Bad "preparing stopped (code $code; output in $log): the game makes the rest itself" }
    # Not again for this build and these recipes either way: a failure would repeat each launch.
    $c.prepared = $c.built_key
    if ($all) { $c.shaders_all = $c.built_key }
    $c.recipes = Recipes-Size
    Save-Config
}


# ------------------------------------------------------------ PC tuning (6.0)
# The PC's graphics chips and power plan, set for the run.
#  - victory.exe runs on the fast card: Windows' own per-app setting
#    (Settings > System > Display > Graphics: "High performance"), so a laptop
#    never makes the game's device on the CPU's own graphics.
#  - With a CPU that has graphics of its own (Intel UHD / Iris / Arc
#    Graphics, AMD Radeon Graphics), the browsers, Discord, Spotify and
#    Steam's web views are moved onto THAT chip ("Power saving"), so they
#    stop keeping hundreds of MB of the NVIDIA card's 6 GB (they take it after
#    their next start). Only programs that had no setting of their own; I
#    switches it off and puts those back.
#  - The "High performance" power plan (and Windows 11's "Best performance"
#    power mode) while the game runs, the old ones back afterwards: on
#    "Balanced" laptops park cores and drop clocks between the title's bursts.
#    VICTORY_TRY_POWER=0: left alone.
$GpuPrefSub = 'Software\Microsoft\DirectX\UserGpuPreferences'
$HelperApps = @('Discord', 'chrome', 'msedge', 'firefox', 'opera', 'brave', 'Spotify', 'steamwebhelper', 'EpicWebHelper')
function GpuPref-Key { return [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($GpuPrefSub) }
function Own-Graphics {
    try {
        $all = @(Get-CimInstance Win32_VideoController -ErrorAction Stop)
        $own = @($all | Where-Object { $_.Name -match 'Intel\(R\) (UHD|Iris|HD) Graphics|Intel\(R\) Graphics|Arc\(TM\) Graphics|Radeon\(TM\) Graphics|Radeon Graphics|Radeon \d{3}M|Vega \d+ Graphics' })
        if ($all.Count -ge 2 -and $own.Count -ge 1) { return [string]$own[0].Name }
    } catch {}
    return ''
}
function Tune-PC {
    $c = $script:Config
    try {
        $key = GpuPref-Key
        $exe = [IO.Path]::GetFullPath((Victory-Exe))
        if ($key -and $exe) {
            if ([string]$key.GetValue($exe) -ne 'GpuPreference=2;') { $key.SetValue($exe, 'GpuPreference=2;') }
            Say 'graphics: victory.exe set to the high-performance card (Windows graphics setting)'
        }
        $own = Own-Graphics
        if ($own -and $c.igpu -ne 'off') {
            $set = @(($c.igpu_set -split '\|') | Where-Object { $_ })
            $paths = @(Get-Process -Name $HelperApps -ErrorAction SilentlyContinue | ForEach-Object { try { $_.Path } catch {} } | Where-Object { $_ } | Sort-Object -Unique)
            $moved = 0
            foreach ($path in $paths) {
                if ($null -eq $key.GetValue($path)) { $key.SetValue($path, 'GpuPreference=1;'); $set += $path; $moved++ }
            }
            $c.igpu_set = (($set | Sort-Object -Unique) -join '|')
            Save-Config
            Say "graphics: the CPU's own graphics ($own) takes the browsers / Discord / Spotify / Steam: $(@($set).Count) program(s), $moved new (they move after they restart; I switches)"
        } elseif ($own) {
            Say "graphics: the CPU's own graphics ($own) is left alone (I switches the helper on)"
        }
        if ($key) { $key.Close() }
    } catch { Bad "could not set the graphics preferences: $_" }
    $script:OldPlan = ''; $script:OldOverlay = ''
    if ($env:VICTORY_TRY_POWER -ne '0') {
        try {
            $now = (& powercfg /getactivescheme 2>$null | Out-String)
            if ($now -match '([0-9a-fA-F]{8}-[0-9a-fA-F-]{27})') { $script:OldPlan = $Matches[1] }
            $high = '8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c'
            if ($script:OldPlan -and $script:OldPlan -ne $high) {
                & powercfg /setactive $high 2>$null | Out-Null
                if ($LASTEXITCODE -ne 0) { $script:OldPlan = '' }
            } else { $script:OldPlan = '' }
            $ov = (& powercfg /getactiveoverlay 2>$null | Out-String)
            if ($LASTEXITCODE -eq 0 -and $ov -match '([0-9a-fA-F]{8}-[0-9a-fA-F-]{27})') {
                $best = 'ded574b5-45a0-4f42-8737-46345c09c238'
                if ($Matches[1] -ne $best) {
                    $script:OldOverlay = $Matches[1]
                    & powercfg /overlaysetactive $best 2>$null | Out-Null
                    if ($LASTEXITCODE -ne 0) { $script:OldOverlay = '' }
                }
            }
            Say "power: $(if ($script:OldPlan -or $script:OldOverlay) { 'High performance while the game runs (the old plan comes back after it)' } else { 'already at its fastest' })"
        } catch {}
    }
}
function Untune-PC {
    try {
        if ($script:OldPlan) { & powercfg /setactive $script:OldPlan 2>$null | Out-Null }
        if ($script:OldOverlay) { & powercfg /overlaysetactive $script:OldOverlay 2>$null | Out-Null }
    } catch {}
    $script:OldPlan = ''; $script:OldOverlay = ''
}
# I off: the programs Tune-PC moved go back to Windows' own choice.
function Undo-Helper {
    $c = $script:Config
    try {
        $key = GpuPref-Key
        foreach ($path in @(($c.igpu_set -split '\|') | Where-Object { $_ })) {
            if ([string]$key.GetValue($path) -eq 'GpuPreference=1;') { $key.DeleteValue($path, $false) }
        }
        $key.Close()
    } catch {}
    $c.igpu_set = ''
}

# 6.7: the saved game. The game saves by itself where you are in the
# mission; VIctory keeps that in the title's folder of its data folder:
# 'saves' (the console's save containers) and 'storage' (its persistent
# local storage). A new game renames both aside (never deletes them), so the
# game starts as the first time it was played: from the very beginning,
# whatever mission the save was at. J brings the last one back.
function Save-Root {
    $d = $env:VICTORY_DATA_DIR
    if (-not $d) { $d = Join-Path $env:LOCALAPPDATA 'victory\data' }
    return (Join-Path $d '7952987A')
}
function Save-Parts {
    $root = Save-Root
    return @(@('saves', 'storage') | ForEach-Object { Join-Path $root $_ } | Where-Object { Test-Path -LiteralPath $_ })
}
function Save-Backups {
    $root = Save-Root
    if (-not (Test-Path -LiteralPath $root)) { return @() }
    return @(Get-ChildItem -LiteralPath $root -Directory -ErrorAction SilentlyContinue | Where-Object { $_.Name -match '^(saves|storage)\.bak-(\d{8}-\d{6})$' } | Sort-Object Name -Descending)
}
function Save-When {
    $parts = Save-Parts
    if (-not $parts) { return '' }
    $f = Get-ChildItem -LiteralPath $parts -Recurse -File -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if ($f) { return $f.LastWriteTime.ToString('yyyy-MM-dd HH:mm') }
    return ''
}
# Renames the save aside: the game starts as the first time.
function Reset-Save {
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $moved = 0
    foreach ($p in (Save-Parts)) {
        $name = (Split-Path -Leaf $p) + ".bak-$stamp"
        try { Rename-Item -LiteralPath $p -NewName $name -ErrorAction Stop; $moved++ }
        catch { Bad "could not put $p aside (is the game still open?): $_"; return $false }
    }
    $script:Config.save_last = "new:$stamp"
    Save-Config
    if ($moved -gt 0) {
        Say "NEW GAME: your save was put aside as *.bak-$stamp in $(Save-Root) (not deleted; J brings it back)."
    } else {
        Say 'NEW GAME: there was no save to put aside.'
    }
    $n = @(Save-Backups).Count
    if ($n -gt 4) { Say "($n saves put aside so far; delete old *.bak-* folders in $(Save-Root) yourself if you need the disk space.)" }
    Say 'The game starts from the very beginning, as the first time you played.'
    return $true
}
# Brings back the save put aside last (the one in use is put aside first).
function Restore-Save {
    $last = Save-Backups | Select-Object -First 1
    if (-not $last) { Bad "no save put aside in $(Save-Root)"; return }
    $stamp = $last.Name -replace '^.*\.bak-', ''
    $now = Get-Date -Format 'yyyyMMdd-HHmmss'
    foreach ($p in (Save-Parts)) {
        try { Rename-Item -LiteralPath $p -NewName ((Split-Path -Leaf $p) + ".bak-$now") -ErrorAction Stop }
        catch { Bad "could not put $p aside (is the game still open?): $_"; return }
    }
    foreach ($part in @('saves', 'storage')) {
        $b = Join-Path (Save-Root) "$part.bak-$stamp"
        if (Test-Path -LiteralPath $b) {
            try { Rename-Item -LiteralPath $b -NewName $part -ErrorAction Stop }
            catch { Bad "could not bring back $b`: $_"; return }
        }
    }
    $script:Config.save_last = "restored:$stamp"
    Save-Config
    Say "your save from $stamp is back (the one you had is put aside as *.bak-$now)."
}
# Asked before every launch (J turns the question off): Enter continues.
function Choose-Save {
    $c = $script:Config
    if ($env:VICTORY_TRY_SAVE -eq 'continue') { return }
    if ($env:VICTORY_TRY_SAVE -eq 'new') { $null = Reset-Save; return }
    if ($c.save_ask -eq 'off') { return }
    Step 'Your saved game'
    $when = Save-When
    if (-not $when) {
        Say 'no saved game yet: the game starts from the beginning.'
        return
    }
    Say "last saved: $when   ($(Save-Root))"
    Write-Host '  [Enter]  CONTINUE   where you left off (the game''s auto-save)' -ForegroundColor Green
    Write-Host '  [N]      NEW GAME   from the very beginning, as the first time you played' -ForegroundColor Yellow
    Say '           (mission 1 even if your save is further on; your save is kept aside, J brings it back)'
    $a = (Read-Host '  Continue or new game').Trim().ToUpper()
    if ($a -eq 'N') {
        $null = Reset-Save
    } else {
        $script:Config.save_last = 'continue'
        Save-Config
        Say 'continuing your saved game'
    }
}
# J: the saved-game menu.
function Save-Menu {
    $c = $script:Config
    Step 'Saved game'
    $when = Save-When
    Say "save folder: $(Save-Root)"
    Say "last saved: $(if ($when) { $when } else { 'no save yet' })"
    $b = @(Save-Backups)
    Say "saves put aside: $(if ($b.Count) { ($b | Select-Object -First 4 | ForEach-Object { $_.Name }) -join ', ' } else { 'none' })"
    Say "asked before every launch: $(if ($c.save_ask -eq 'off') { 'no' } else { 'yes' })"
    Say '[N] new game at the next launch (puts the save aside now)   [B] bring back the save put aside last'
    Say '[A] ask before every launch on / off                          [Enter] back'
    $a = (Read-Host '  Choice').Trim().ToUpper()
    switch ($a) {
        'N' { $null = Reset-Save }
        'B' { Restore-Save }
        'A' { $c.save_ask = $(if ($c.save_ask -eq 'off') { '' } else { 'off' }); Save-Config; Say "asked before every launch: $(if ($c.save_ask -eq 'off') { 'no' } else { 'yes' })" }
        default {}
    }
}

function Run-Game {
    $c = $script:Config
    Choose-Save
    Step 'Running the game with VIctory'
    Say "game: $($c.game_exe)"
    # What victory.exe was built from (not just what was downloaded): a
    # failed build keeps the last working one.
    $built = ''
    if ("$($c.built_key)" -match '^fork:([0-9a-f]{7,40})') { $built = $Matches[1] }
    $fetched = (G rev-parse -q --verify refs/remotes/origin/fork) | Select-Object -First 1
    if (EngineAvm) {
        $built = ''
        if ("$($c.builta)" -match '^engineavm:([0-9a-f]{7,40})') {
            $line = (GA log -1 --format='%h %ad %s' --date=short $Matches[1]) | Select-Object -First 1
            Say "VIctory: the AvatarManz engine, built from $line"
        }
    }
    if (Engine8) {
        $built = ''
        if ("$($c.built8)" -match '^engine8:([0-9a-f]{7,40})') {
            $line = (G8 log -1 --format='%h %ad %s' --date=short $Matches[1]) | Select-Object -First 1
            Say "VIctory: the 8 GB NVIDIA engine, built from $line"
        }
    }
    if ($built) {
        $line = (G log -1 --format='%h %s' $built) | Select-Object -First 1
        Say "VIctory: built from $line"
    }
    if ($built -and $fetched -and ($built -ne $fetched)) {
        $new = (G log -1 --format='%h %s' $fetched) | Select-Object -First 1
        Bad "this is an OLDER build: the newest fork ($new) did not build."
        Bad "Press U to try again; if it fails, send $(Join-Path $LogDir 'build.log')"
    }
    if ($c.added) { Say "plus upstream: $(@($c.added -split "`n").Count) new fix(es) on top" }
    if ($c.console -ne 'series-x') { $c.console = 'series-s' }
    Auto-Console
    # 5.7: VIctory itself runs 6 and 8 GB cards as Series S when Series X is
    # asked for; a Series X picked with C stays Series X.
    $env:VICTORY_CONSOLE_AUTO = $(if ($c.console_user -eq '1') { '0' } else { '' })
    Say "console: Xbox $(if ($c.console -eq 'series-s') { 'Series S' } else { 'Series X' })   (C switches)"
    Say "command: victory.exe run `"$($c.game_exe)`" --execute --console $($c.console)   (VICTORY_GAME_DIR=$($c.game_dir))"
    Say 'To stop: close the game window.'
    # (6.9: none of this fork's settings for the AvatarManz engine.)
    if (-not (EngineAvm)) { Set-RunEnv }
    # 6.16: failed shaders older than 3 days go (each run and prepare saves
    # its own again; recent ones stay for the report).
    if ((OwnEngine) -and $script:Config.tidy -ne 'off' -and $env:VICTORY_DUMP_FAILED -and (Test-Path -LiteralPath $env:VICTORY_DUMP_FAILED)) {
        $old = (Get-Date).AddDays(-3)
        Get-ChildItem -LiteralPath $env:VICTORY_DUMP_FAILED -File -Recurse -Force -ErrorAction SilentlyContinue | Where-Object { $_.LastWriteTime -lt $old } | Remove-Item -Force -ErrorAction SilentlyContinue
    }
    Tune-PC
    if (EngineAvm) { Set-EngineAvmEnv; Prepare-Avm 'new build or new pipelines' }
    elseif (Engine8) { Set-Engine8Env }
    elseif ("$($c.prepared)" -ne "$($c.built_key)") { Prepare-Shaders 'new build' }
    elseif ((OwnEngine) -and "$($c.shaders_all)" -ne "$($c.built_key)" -and $env:VICTORY_TRY_ALL_SHADERS -ne '0') { Prepare-Shaders 'every shader of the game, once per build' }
    elseif ("$($c.recipes)" -ne (Recipes-Size)) { Prepare-Shaders 'new pipelines from the last run' }
    # The shared pipeline list (keys of pipelines other players met, no
    # game files): the game starts making them while it loads.
    $shared = Join-Path $Src 'scripts\try\shared-pipelines.txt'
    if ((OwnEngine) -and (Test-Path -LiteralPath $shared)) {
        $pdir = Join-Path $env:VICTORY_CACHE_DIR 'pipelines'
        try {
            New-Item -ItemType Directory -Force -Path $pdir | Out-Null
            Copy-Item -LiteralPath $shared -Destination (Join-Path $pdir 'shared-pipelines.txt') -Force
            $n = @(Get-Content -LiteralPath $shared | Where-Object { $_ -and -not $_.StartsWith('#') }).Count
            Say "shared pipeline list: $n pipelines other players met are made while the game loads"
        } catch { Bad "could not copy the shared pipeline list: $_" }
    }
    if (OwnEngine) {
        Say "failed shaders saved to: $($env:VICTORY_DUMP_FAILED)"
        Say "shader loop limit: $($env:VICTORY_SHADER_LOOP_LIMIT)   skipped programs: $($env:VICTORY_SKIP_PIPELINES)"
    }
    Say 'New places compile their pipelines while you play (things may be missing for a while); after you close the game they are all compiled, and the next launch has them from the start.'
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $log = Join-Path $LogDir "game-$stamp.log"
    # 4.3: the game's output (the rendering lines) goes to a window of its
    # own; this one keeps the menu, versions and settings.
    # VICTORY_TRY_ONE_WINDOW=1: everything in this window, as before.
    $code = $null
    if ($env:VICTORY_TRY_ONE_WINDOW -ne '1') {
        $code = Run-InOwnWindow $c $log
    }
    if ($null -eq $code) {
        Push-Location $TryHome
        try {
            & (Victory-Exe) run $c.game_exe --execute --console $c.console 2>&1 | ForEach-Object { "$_" } | Tee-Object -FilePath $log
            $code = $LASTEXITCODE
        } finally { Pop-Location }
    }
    Untune-PC
    Write-Host ''
    Say "the game stopped (exit code $code)"
    Say "its output: $log"
    $crashes = Join-Path $env:LOCALAPPDATA 'victory\data\crashes.log'
    if (Test-Path $crashes) { Say "crash details, if it crashed: $crashes" }
    try { Check-Run $log $stamp $crashes } catch { Bad "could not summarize the run: $_" }
    # Everything this run asked for and did not get, compiled now while the
    # game is closed: the next launch shows it from the start.
    if ((OwnEngine) -and "$($c.recipes)" -ne (Recipes-Size)) { Prepare-Shaders 'new pipelines from this run' }
    if (EngineAvm) { Prepare-Avm 'new pipelines from this run' }
}

# The game in a window of its own ('VIctory - game output'), with its output
# saved to $log as before. Returns the exit code, or $null if the window could
# not be opened (the caller then runs the game here).
function Run-InOwnWindow($c, [string]$log) {
    $runner = Join-Path $TryHome 'run-game.ps1'
    $codeFile = "$log.exit"
    $lines = @(
        'param([string]$Exe, [string]$Game, [string]$Console, [string]$Log, [string]$CodeFile)',
        'try { $Host.UI.RawUI.WindowTitle = ''VIctory - game output'' } catch {}',
        "Write-Host '== VIctory Try $TryVersion - game output (the menu is in the VIctory Try window)' -ForegroundColor Cyan",
        'Write-Host ''   This window closes by itself when the game stops.''',
        'Write-Host ''''',
        '& $Exe run $Game --execute --console $Console 2>&1 | ForEach-Object { "$_" } | Tee-Object -FilePath $Log',
        '$code = $LASTEXITCODE',
        'Set-Content -LiteralPath $CodeFile -Value ([string]$code) -Encoding ASCII',
        'exit $code'
    )
    try {
        Set-Content -LiteralPath $runner -Value $lines -Encoding ASCII
        Remove-Item -LiteralPath $codeFile -Force -ErrorAction SilentlyContinue
        $ps = (Get-Process -Id $PID).Path
        $q = { param($v) '"' + ([string]$v).Replace('"', '') + '"' }
        $argLine = "-NoProfile -ExecutionPolicy Bypass -File $(& $q $runner) $(& $q (Victory-Exe)) $(& $q $c.game_exe) $(& $q $c.console) $(& $q $log) $(& $q $codeFile)"
        $p = Start-Process -FilePath $ps -ArgumentList $argLine -WorkingDirectory $TryHome -PassThru
    } catch {
        Bad "could not open the game output window ($_): the game runs in this window"
        return $null
    }
    Say 'the game is running; its output is in the other window (VIctory - game output).'
    Say 'Close the game window to come back here.'
    $p.WaitForExit()
    $code = $null
    if (Test-Path -LiteralPath $codeFile) {
        $code = (Get-Content -LiteralPath $codeFile -ErrorAction SilentlyContinue | Select-Object -First 1)
        Remove-Item -LiteralPath $codeFile -Force -ErrorAction SilentlyContinue
    }
    if (-not $code) { try { $code = $p.ExitCode } catch {} }
    if ($null -eq $code -or "$code" -eq '') { $code = 'unknown' }
    return $code
}

# After a run: a short summary of its log, the safer setting for next time
# if the card was lost (only then), and one zip with everything to send.
# The card's memory budget in MB the last game log names (VICTORY's
# "vram_budget_mb=", coloured), or 0.
function Log-Vram([string]$log) {
    if (-not $log -or -not (Test-Path -LiteralPath $log)) { return 0 }
    $m = Select-String -LiteralPath $log -Pattern 'vram_budget_mb(?:\x1b\[[0-9;]*m|=)*([0-9]+)' -List -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($m -and $m.Matches.Count -gt 0) { return [int]$m.Matches[0].Groups[1].Value }
    # 5.6: the game log is UTF-16 without a mark, which Select-String
    # misreads: read the bytes and decode them ourselves.
    try {
        $bytes = [IO.File]::ReadAllBytes($log)
        $n = [Math]::Min($bytes.Length, 4MB)
        $text = if ($n -gt 1 -and $bytes[1] -eq 0) { [Text.Encoding]::Unicode.GetString($bytes, 0, $n - ($n % 2)) } else { [Text.Encoding]::UTF8.GetString($bytes, 0, $n) }
        $mm = [regex]::Match($text, 'vram_budget_mb(?:\x1b\[[0-9;]*m|=)*([0-9]+)')
        if ($mm.Success) { return [int]$mm.Groups[1].Value }
    } catch {}
    return 0
}
# 5.6: the card's own memory in MB from the driver's registry entry (the
# largest card), or 0. Known before any game log exists.
function Card-Vram {
    $best = [int64]0
    try {
        Get-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\0*' -ErrorAction SilentlyContinue | ForEach-Object {
            $q = $_.'HardwareInformation.qwMemorySize'
            if ($q -and [int64]$q -gt $best) { $best = [int64]$q }
        }
    } catch {}
    return [int]($best / 1MB)
}
# 5.5: cards with less than 8 GB of budget (6 and 8 GB cards) run as Series S
# once by themselves. Series X's render targets took 2.3 to 3 GB of them, and
# the textures that no longer fitted waited, drawn black (roads, buildings,
# people; 17,000 to 27,000 waiting in the 2026-11-25 and 11-26 reports).
# Series S renders at 1280x720 and asks for less memory. C goes back, and
# after that the choice stays yours.
function Auto-Console {
    $c = $script:Config
    if ($c.console_auto -eq '5.6') { return }
    if (-not $c.vram) {
        $last = Get-ChildItem -LiteralPath $LogDir -Filter 'game-*.log' -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1
        if ($last) { $v = Log-Vram $last.FullName; if ($v -gt 0) { $c.vram = [string]$v } }
    }
    # 5.6: no budget from a log (5.5 never found it): the card's own memory.
    $card = Card-Vram
    $small = ($c.vram -and [int]$c.vram -lt 8000) -or ($card -gt 0 -and $card -le 9000)
    if (-not $c.vram -and $card -le 0) { return }
    $c.console_auto = '5.6'
    if ($small -and $c.console -eq 'series-x') {
        $c.console = 'series-s'
        $what = $(if ($c.vram) { "memory budget is $($c.vram) MB" } else { "memory is $card MB" })
        Say "your card's $what`: switched to Series S (less card memory, so textures load instead of staying black). C goes back to Series X."
    }
    Save-Config
}

function Check-Run([string]$log, [string]$stamp, [string]$crashes) {
    if (-not (Test-Path $log)) { return }
    $c = $script:Config
    $lines = @(Get-Content -LiteralPath $log -ErrorAction SilentlyContinue)
    $count = { param($pattern) @($lines | Where-Object { $_ -match $pattern }).Count }
    $fps = @()
    foreach ($l in $lines) { if ($l -match '([0-9]+\.[0-9]) fps, [0-9.]+ ms/frame') { $fps += [double]$Matches[1] } }
    Step 'Run summary'
    if ($fps.Count -gt 0) {
        $last = @($fps | Select-Object -Last 20 | Sort-Object)
        $median = $last[[int][math]::Floor($last.Count / 2)]
        $best = ($fps | Measure-Object -Maximum).Maximum
        Say "speed: $median fps typical at the end (last $($last.Count) reports), $best fps best"
    } else {
        Say 'speed: no frame report in the log (the world was not reached, or the run was short)'
    }
    # 5.5: the card's budget (for Auto-Console) and the textures still
    # waiting for room at the end (drawn black until they load).
    $v = Log-Vram $log
    if ($v -gt 0) { $c.vram = [string]$v; Save-Config }
    $lastWait = @($lines | Where-Object { $_ -match 'picked by index: [0-9]+ uploaded so far \([0-9]+ MB\), ([0-9]+) waiting' }) | Select-Object -Last 1
    if ($lastWait -and $lastWait -match ', ([0-9]+) waiting') {
        $w = [int]$Matches[1]
        if ($w -gt 2000) { Say "textures waiting at the end: $w (black until they load: the card's memory is full$(if ($c.console -eq 'series-x') { '; Series S (C) needs less' }))" }
        else { Say "textures waiting at the end: $w" }
    }
    # Draws still waiting for their pipelines at the end: what is missing
    # (see-through objects) until the next launches have compiled them.
    $lastFrame = @($lines | Where-Object { $_ -match 'dropped ([0-9]+) without a host pipeline \(([0-9]+) being made' }) | Select-Object -Last 1
    if ($lastFrame -and $lastFrame -match 'dropped ([0-9]+) without a host pipeline \(([0-9]+) being made') {
        if ($Matches[2] -eq '0') { Say "at the end: $($Matches[1]) draws a frame skipped (compute work, not objects: mostly the ray-tracing builder, which is off, and the programs counted below)" }
        else { Say "at the end: $($Matches[1]) draws a frame still waiting for $($Matches[2]) pipelines being compiled (fewer each launch: run it again)" }
    }
    # 4.8: which programs those draws belong to and why (every build logs
    # it now): the last value of each reason, most first.
    $why = @{}
    foreach ($l in @($lines | Where-Object { $_ -match 'dropped [0-9.]+/frame without a host pipeline: ' } | Select-Object -Last 36)) {
        if ($l -match 'dropped ([0-9.]+)/frame without a host pipeline: (.+?)\s*$') {
            $why[$Matches[2]] = [double]::Parse($Matches[1], [Globalization.CultureInfo]::InvariantCulture)
        }
    }
    if ($why.Count -gt 0) {
        Say 'draws a frame not drawn at the end, and why:'
        $why.GetEnumerator() | Sort-Object Value -Descending | Select-Object -First 6 | ForEach-Object {
            $text = "$($_.Key)"
            if ($text.Length -gt 110) { $text = $text.Substring(0, 110) + '...' }
            Say ('  {0,7}  {1}' -f $_.Value.ToString('0.0', [Globalization.CultureInfo]::InvariantCulture), $text)
        }
    }
    Say "shaders that do not translate: $(& $count 'shader does not translate')"
    Say "pipelines the card refused: $(& $count 'host pipeline refused')"
    Say "slow submissions (over 250 ms on the card): $(& $count 'slow host submission')"
    # A crash on a memory access (0xc0000005) with lazy copy-back on: the
    # next run copies back first, as before 3.9.
    # 6.9: the AvatarManz engine's own fallbacks: a lost card first turns on
    # its -Safe set, then its renderer patches off; memory that ran out is said.
    if (EngineAvm) {
        if ((& $count 'out of memory|E_OUTOFMEMORY|0x8007000e|commit limit|paging file is too small') -gt 0) {
            Bad 'the PC ran out of memory in this run: give Windows a bigger page file (System managed, or 49152 MB) and restart the PC.'
        }
        if ((& $count 'device removed|DEVICE_REMOVED|DEVICE_HUNG') -gt 0) {
            Bad 'the graphics card was lost (device removed) in this run.'
            if ($c.rt -eq '1') { $c.rt = ''; Say 'next run: ray tracing off (B switches it again).' }
            elseif ($c.safe -ne '1') { $c.safe = '1'; Say 'next run: Safe mode (Z switches it off again).' }
            elseif ($c.ports -ne 'off') { $c.ports = 'off'; Say 'next run: renderer patches off (Y switches them on again).' }
            Save-Config
        }
    }
    $faulted = & $count 'VIctory: .*0xc0000005'
    if ((OwnEngine) -and $faulted -gt 0 -and $script:LazyFromTry) {
        $c.lazy = 'off'
        Save-Config
        Bad 'the game crashed on a memory access in this run.'
        Say 'next run: lazy copy-back is off (the card results are copied back before the game hears the card is done).'
    }
    # 6.7: the descriptor slots ran out with 384 lists in the air: next run 256.
    if ((OwnEngine) -and (& $count 'dynamic descriptor slots ran out|no free dynamic descriptor slots') -gt 0 -and $c.slots -ne 'out') {
        $c.slots = 'out'
        Save-Config
        Say 'next run: 256 lists in the air before a wait (the descriptor slots ran out with 384).'
    }
    $lost = & $count 'device removed|DEVICE_REMOVED|DEVICE_HUNG'
    if ((OwnEngine) -and $lost -gt 0) {
        Bad 'the graphics card was lost (device removed) in this run.'
        if (AllOut) {
            # 6.12: all-out mode is the first thing to go back (it fills the card to 99%).
            $c.allout = ''
            Say 'next run: all-out mode is off (the card was filled to its edge; MAX switches it again).'
        } elseif ($script:BesideFromTry) {
            # 6.17: the newest change goes next.
            $c.beside = 'off'
            Say 'next run: the descriptor heap pass runs on the submitting thread again (6.17 beside off).'
        } elseif ($script:BarWindowsMb -gt 0 -and (& $count 'windows through the BAR') -gt 0) {
            # 6.13: then the memory windows on the card through the BAR.
            $c.barwin = 'off'
            Say 'next run: memory windows stay in the PC''s memory (6.13 Resizable BAR windows off).'
        } elseif ($script:WideHashFromTry) {
            $c.widehash = 'off'
            Say 'next run: the card hashes copies one thread a piece again (6.14 wide hashes off).'
        } elseif ($script:AheadFromTry) {
            $c.ahead = 'off'
            Say 'next run: the game waits for its own submissions and present again (6.14 one-frame-ahead off).'
        } elseif ($script:StandInFromTry) {
            $c.standin = 'off'
            Say 'next run: draws of new objects wait for their own pipeline again (6.15 stand-ins off).'
        } elseif ($c.fast -eq '1') {
            $c.fast = ''
            Say 'next run: back to the stable relay speed mode (F switches again).'
        } elseif ($script:FxFromTry) {
            $c.fxspeed = 'off'
            Say 'next run: fxceless''s speed set (6.8) is off (Y switches it again).'
        } elseif ($script:InflightFromTry) {
            $c.inflight = 'off'
            Say 'next run: card results are fetched the old way (6.6 in-flight fetch off).'
        } elseif ($script:RebarFromTry) {
            $c.rebar = 'off'
            Say 'next run: Resizable BAR uploads (6.4) are off.'
        } elseif ($script:NewFromTry) {
            $c.new38 = 'off'
            Say 'next run: the 3.8 card addition (PC-side card copies) is off.'
        } elseif ($script:WaveWideFromTry) {
            $c.wavewide = 'off'
            Say 'next run: the 64-lane compute shaders are left out again, as before 3.6.'
        } elseif ($script:WideFromTry) {
            $c.wide = 'off'
            Say 'next run: copy-back on one thread again, as before 3.5.'
        } elseif ($script:LoopFromTry -and $c.loop -ne '128') {
            $c.loop = '128'
            Say 'next run: shader loop limit 128 instead of 256 (fewer card timeouts; long effects may look a bit off).'
        } elseif ($script:TrimFromTry) {
            $c.trim = 'off'
            Say 'next run: every barrier is made again (barrier trimming off).'
        } elseif ($script:CopyFetchFromTry) {
            $c.copyfetch = 'off'
            Say 'next run: card results are fetched behind the flights again (copy-engine fetches off).'
        }
        Save-Config
    }
    # Everything to send in one file, in the reports folder (S changes it).
    # (6.1: the version in the name, and in try-version.txt inside.)
    $zip = Join-Path (Reports-Dir) "VIctory-report-$TryVersion-$stamp.zip"
    $stage = Join-Path $TryHome "report-$stamp"
    try {
        New-Item -ItemType Directory -Force -Path $stage | Out-Null
        Copy-Item -LiteralPath $log -Destination $stage
        $engineLine = $(if (EngineAvm) { "engine: AvatarManz ($($c.builta)) $EngineAvmUrl $EngineAvmBranch" } elseif (Engine8) { "engine: 8 GB ($($c.built8))" } else { "engine: ours ($($c.built_key))" })
        # 6.9: the settings the run had (every VICTORY_ one) and the PC's memory.
        $ram = Ram-Info
        $envLines = @(Get-ChildItem Env: | Where-Object { $_.Name -like 'VICTORY_*' } | Sort-Object Name | ForEach-Object { "$($_.Name)=$($_.Value)" })
        Set-Content -LiteralPath (Join-Path $stage 'settings.txt') -Value (@("ram_mb=$($ram[0]) commit_limit_mb=$($ram[1]) card_mb=$(Card-Vram) threads=$([Environment]::ProcessorCount)") + $envLines) -Encoding ASCII
        $prep = Join-Path $LogDir 'prepare.log'
        if (Test-Path -LiteralPath $prep) { Copy-Item -LiteralPath $prep -Destination $stage }
        Set-Content -LiteralPath (Join-Path $stage 'try-version.txt') -Value @("VIctory Try $TryVersion", $engineLine, "console: $($c.console)") -Encoding ASCII
        if (Test-Path $crashes) { Copy-Item -LiteralPath $crashes -Destination $stage }
        # This PC's pipeline list (keys only), for the shared list.
        $early = Join-Path $env:VICTORY_CACHE_DIR 'pipelines\early-pipelines.txt'
        if (Test-Path -LiteralPath $early) { Copy-Item -LiteralPath $early -Destination $stage }
        $buildLog = Join-Path $LogDir 'build.log'
        if (Test-Path $buildLog) { Copy-Item -LiteralPath $buildLog -Destination $stage }
        $dump = $env:VICTORY_DUMP_FAILED
        if ($dump -and (Test-Path $dump)) {
            $size = (Get-ChildItem -LiteralPath $dump -Recurse -File -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
            if ($size -lt 300MB) { Copy-Item -LiteralPath $dump -Destination (Join-Path $stage 'failed-shaders') -Recurse }
            else { Say "failed shaders left out of the zip ($([int]($size / 1MB)) MB): zip $dump yourself if asked" }
        }
        Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -Force
        Say "everything to send is in one file: $zip"
    } catch {
        Bad "could not make the report zip ($_): send $log instead"
    } finally {
        Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
    }
}

# ------------------------------------------------------------- main

Write-Host ''
Write-Host "  VIctory Try $TryVersion" -ForegroundColor Green
try { $Host.UI.RawUI.WindowTitle = "VIctory Try $TryVersion" } catch {}
Say 'Builds the newest VIctory from MfDev0/victory, with upstream VIctory''s new'
Say 'fixes added by themselves, on this PC and runs your game.'
Say 'Nothing is uploaded or reported anywhere.'

# Started from inside a game run (the game path pointed at VIctory Try, or a
# title started it): stop instead of running again inside it.
if ($env:VICTORY_TRY_RUNNING -eq '1' -and $env:VICTORY_TRY_HANDOFF -ne '1') {
    Bad 'VIctory Try was started from inside a game run: stopping this copy.'
    Bad 'If this repeats, press G in the first VIctory Try window and give the game''s own exe.'
    exit 3
}
$handoff = $env:VICTORY_TRY_HANDOFF -eq '1'
$env:VICTORY_TRY_HANDOFF = ''
$env:VICTORY_TRY_RUNNING = '1'

# One VIctory Try at a time: two would build in the same folders.
$script:Lock = New-Object System.Threading.Mutex($false, 'Local\VIctoryTry')
$owned = $false
# An abandoned lock (a copy that was killed) is ours now.
try { $owned = $script:Lock.WaitOne(0) } catch { $owned = $true }
if (-not $owned) {
    Bad 'VIctory Try is already running in another window: use that one, or close it first.'
    exit 4
}

# A newer VIctory Try on the branch than this one: run that instead, so its
# fixes need no new download of VIctoryTry.exe.
function Run-Newer([switch]$FromMenu) {
    if ($handoff -and -not $FromMenu) { return }
    $text = (G show refs/remotes/origin/fork:scripts/try/try.ps1) -join "`r`n"
    if ($script:GCode -ne 0 -or $text -notmatch "\`$TryVersion = '([0-9.]+)'") { return }
    $theirs = $Matches[1]
    try { if ([version]$theirs -le [version]$TryVersion) { return } } catch { return }
    Step "UPDATING: starting VIctory Try $theirs (this window ran $TryVersion)"
    $newer = Join-Path $TryHome 'try-latest.ps1'
    Set-Content -LiteralPath $newer -Value $text -Encoding ASCII
    $script:Lock.ReleaseMutex()
    # 6.1: the newer one starts from the settings made before VIctory Try
    # started, not from the ones this window set for its runs (it took those
    # for the user's own and kept this version's defaults).
    if ($script:UserAll) {
        Get-ChildItem Env: | Where-Object { $_.Name -like 'VICTORY_*' -and -not $script:UserAll.ContainsKey($_.Name) } | ForEach-Object { Remove-Item -LiteralPath "Env:$($_.Name)" -ErrorAction SilentlyContinue }
        foreach ($k in $script:UserAll.Keys) { [Environment]::SetEnvironmentVariable($k, $script:UserAll[$k], 'Process') }
    }
    $env:VICTORY_TRY_RUNNING = ''
    $env:VICTORY_TRY_HANDOFF = '1'
    $ps = (Get-Process -Id $PID).Path
    & $ps -NoProfile -ExecutionPolicy Bypass -File $newer
    exit $LASTEXITCODE
}

$script:Config = Load-Config
# 5.0: every run starts from the recommended settings once (what renders the
# most on an 8 GB NVIDIA card: the stable relay, copy-back on several threads,
# 64-lane programs translated, lazy copy-back, levels on demand, ambient light,
# shadow fix, copies on the card, clean-up off, fps boost), whatever earlier
# versions or a lost card switched off. D does the same at any time.
function Reset-Recommended {
    foreach ($k in @('fast', 'loop', 'wide', 'wavewide', 'new38', 'lazy', 'plain', 'boost', 'between', 'hybrid', 'ambient', 'levels', 'opt', 'shadowfix', 'gpucopy', 'temporal', 'fxspeed', 'low', 'allout')) { $script:Config[$k] = '' }
    $script:Config.defaults = '5.0'
    Save-Config
}
if ($script:Config.defaults -ne '5.0') {
    Reset-Recommended
    Say 'settings: back to the recommended ones for 5.0 (D does this again any time)'
}
$script:UserLoop = $env:VICTORY_SHADER_LOOP_LIMIT
$script:UserWaveWide = $env:VICTORY_WAVE_WIDE
$script:UserDisplay = $env:VICTORY_DISPLAY_MODE
$script:UserFx = @{}
# 5.9: every VICTORY_ setting the user made before VIctory Try started (the
# 8 GB engine starts from these alone).
$script:UserAll = @{}
Get-ChildItem Env: | Where-Object { $_.Name -like 'VICTORY_*' -and $_.Name -ne 'VICTORY_TRY_HANDOFF' } | ForEach-Object { $script:UserAll[$_.Name] = $_.Value }
foreach ($name in @('VICTORY_RENDER_SCALE', 'VICTORY_SKIP_RT', 'VICTORY_FRAMEGEN', 'VICTORY_INPLACE', 'VICTORY_AMBIENT', 'VICTORY_LEVELS_ON_DEMAND', 'VICTORY_POOL_NO_ASSERT', 'VICTORY_SHADER_OPT', 'VICTORY_WAVE_DEEP_SKIPS_NOT', 'VICTORY_GPU_COPIES', 'VICTORY_CARD_REHASH_RUNS', 'VICTORY_INDIRECT_WINDOW', 'VICTORY_JOIN_COPIES', 'VICTORY_CARD_AWAKE')) { $script:UserFx[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
while (-not (Ask-Game $false)) {}
if (-not (Ensure-Tools)) { exit 1 }
$changed = Update-Source
if ($null -eq $changed) { Bad 'no source to build: check the internet connection'; exit 1 }
Run-Newer
if (OwnEngine) {
    # 6.17: a failed build of a new version no longer stops VIctory Try when
    # an older build is there: the game runs on it (the build is tried again
    # at the next start, the version's key not being saved).
    if ($changed -or -not (Test-Path (Ours-Exe))) {
        if (-not (Build-All)) {
            if (-not (Test-Path (Ours-Exe))) { exit 1 }
            Bad 'the new build FAILED: the game runs on the last working build (older fixes). Send the error above (build.log is in the report).'
        }
    }
} elseif (-not (Ensure-Engine $true)) { exit 1 }
Tidy-Disk ([bool]$changed)
if (OwnEngine) {
    Say '6.22: from ScarlettPort (the VIctory port that runs 7-22 fps on cards like yours): no frame stands still for a new shader any more (objects come in a frame or two later instead), the game''s small buffers go to the card''s memory, the upscaler''s network stays on the card, fewer descriptor heap re-sets, cheaper write tracking, two shader clean-ups that are exact (on), and saves no longer wait. The water''s light shaders in their exact form are on by default now (no blocks, VICTORY_WAVE_PAIRS_ONLY=none goes back). AMD: mesh pipelines made one at a time (RX 6000 lost the card in the intro).'
    Say '6.21: from fxceless''s fork (RTX 4070): about 30 of his fixes, among them a dispatch now waits for its compute shader instead of being dropped, mesh draws started by the card, busy memory pages patched on the card instead of waited for, and the water''s light shaders in an exact form (opt-in). Kept off here: his ray-traced shadows (0.2 to 3 fps even on his 4070); our fill light stays, so shadows are not black. Five ray-tracing shaders now translate with default settings.'
    Say '6.20: a clean log: on 6 GB cards MAX leaves about 650 MB more of the card to textures (it stood past its budget, so textures found no room and Windows moved memory to the PC), five ray-tracing compute shaders that were dropped translate, constant buffers and raw views the game places off the PC''s alignment are no longer read as zeros, a false alarm no longer makes every later frame wait, and what is on purpose no longer shows as a warning.'\r\n    Say '6.19: the picture: the light the game''s compute shaders write into memory it placed itself is kept (it went nowhere: probes and bounced light read memory nothing wrote), open water without dark blocks (its tiled light left out on NVIDIA), textures that come back into view are kept (no more white or grey people), screen copies the game gave up are let go, a third texture level goes while the card is past its memory, 1.5 s after a loading screen at most; the log says more.'
    Say '6.17: the fps: the game''s fence wait (monitorx) is patched in place now: it could not be where it lay across two 8-byte words, and took an exception 440,000 times a second (more than a whole processor); the pass over the game''s descriptor heap (22-39 ms a frame) runs beside the preparing instead of before it.'
    Say '6.16: the fps: the textures had filled the card, so the game''s memory windows stayed in the PC''s memory and the card read them across the bus (about 200 ms of card work a frame on a 6 GB laptop card); room on the card is kept for them now and all-out leaves 6% of a small card free. The texture rush no longer goes on and off every second (the same textures uploaded again and again, and faces flicking between skin, white and see-through). The disk: older builds'' shader translations, the other engines and old logs are removed (TIDY shows what takes what). The card''s time is measured in every report.'
    Say '6.13: NVIDIA fixes from the other forks: on PCs under 24 GB of memory the game''s memory windows go straight into the card through Resizable BAR (out-of-memory and the driver crash gone, 1.6-3 -> 10-12 fps on a 16 GB RTX 3060 PC; Indy2l), texture read-backs on their own thread (no more 100-400 ms landing stalls while the world streams), a slow shader is waited for in one frame only (no more freezes while it compiles), compute shaders start compiling as soon as the game makes them, face decorations found by name (no black faces in cutscenes), streamed textures are no longer forgotten when the change list fills, and the card''s buffer compare goes out on its own (D3D12 error 538).'
    Say '6.12: MAX: all-out mode (no caps: every CPU thread but one at high priority, textures up to 99% of the card, the memory guard at 1% free, read from this PC); the ray-tracing light passes that made rooms and reflections white and hazy are left out (fewer pipelines to compile too), and NaN colours no longer spread into black or blocky patches (both from trustno1-coder/VIctory-NVIDIA).'
    Say '6.11: shader stutter: the pipeline library is read into memory ahead on 16 GB PCs too (it was skipped: every pipeline in play waited on the disk), every shader of the game translated once per build with the game closed; joined submissions, copy-back split finer over the cores (lepotane), anisotropic filtering keeps point mips (AvatarManz).'
    Say '6.10: this fork''s engine again (the base stays; the others'' fixes come into it): AMD''s PC FidelityFX upscaler makes the picture (FSR 3.1 on NVIDIA; T switches), sharper own upscaler without blocks on lit surfaces, shaders AMD''s compiler made are read, the game''s debug shaders stored without packing too.'
}
if (EngineAvm) {
    Say '6.9: the AvatarManz engine (PortWatch #1 and #2, 16-22 fps on their cards): renderer patches, list calls on eight threads, textures on their own queue, every shader translated and every pipeline made with the game closed (no compiling while you play). N switches to this fork''s engine.'
} elseif (Engine8) {
    Say 'engine: the 8 GB NVIDIA engine (soldsawn/victory-nvidia-8gb). N goes back to this fork''s engine.'
} else {
    Say '6.7: before the game loads: continue your save or start a NEW GAME from the very beginning (J: saved game); hair and soft shadows accumulated over frames as the console''s upscaler does (T); more work in flight before waiting for the card, card results fetched inside their flights on their own switch, more buffers compared on the card, the upscaler DLL no longer refused.'
    Say '6.6: this fork''s engine: texture threads no longer queue on one lock, results read one frame late instead of waiting for the card, card copies come back inside their own flights, fill light follows the scene''s brightness; 6.5: streaming gets a fixed share of each frame (as big games do), a texture-pass profiler, steadier texture quality; 6.4: the world waits behind its curtain until its textures are in, small textures first, reused upload buffers, shader cache read ahead, softer light on people; the 6.0 speed fixes (all CPU cores, the card''s queues first, the CPU''s own graphics for other programs). N switches to the 8 GB engine.'
}

# Every command of the menu: its key, what it is, whether it is on now, and
# what it does (4.6).
function Show-Commands {
    $c = $script:Config
    $on = { param($b) if ($b) { 'ON ' } else { 'off' } }
    if (EngineAvm) {
        $rows = @(
            @('Enter', 'Run again', '', 'start the game again with the settings below'),
            @('U', 'Update', '', 'download the newest engine, build it, then run the game'),
            @('N', 'Engine', $(Engine-Name), 'own: this fork''s (default, the others'' fixes ported in); avm: AvatarManz/Belobug''s; 8GB: soldsawn''s'),
            @('V', 'Version', '', 'show which Try and VIctory version you have'),
            @('G', 'Game paths', '', 'ask again where the game folder and game_scarlett_master.exe are'),
            @('C', 'Console', $(if ($c.console -eq 'series-s') { 'S  ' } else { 'X  ' }), 'Series S (renders 1280x720 to 1080p: faster, the one for 6 GB cards) or Series X'),
            @('R', 'Output size', $(if ($c.res -eq '4k') { '4K ' } else { '1080' }), 'Series X only: the size the game renders to (1080p or 4K)'),
            @('Y', 'Renderer patches', $(& $on ($c.ports -ne 'off')), 'AvatarManz''s 5 patches of the game''s renderer (faster; off if something looks wrong)'),
            @('B', 'Ray tracing', $(& $on ($c.rt -eq '1')), 'the game''s ray-traced light: lit rooms, right reflections; about a third slower'),
            @('X', 'Textures', $(switch ($c.texcap) { 'sharp' { 'shp' } 'low' { 'low' } default { 'aut' } }), 'auto: sharp while the card has room; sharp: up to 2048 always (faces); low: 512 (6 GB stutter)'),
            @('A', 'Ambient light', $(switch ($c.ambient) { 'off' { 'off' } 'low' { 'low' } 'high' { 'hi ' } default { 'ON ' } }), 'soft light where the sun does not reach: normal, low, high, off'),
            @('F', 'Above 30 fps', $(& $on ($c.unlock -eq '1')), 'lets the game go above 30 fps (menus reach 60; the game clock stays right)'),
            @('Z', 'Safe mode', $(& $on ($c.safe -eq '1')), 'AvatarManz''s -Safe: without the newest, faster ways (first thing to try if it breaks)'),
            @('I', 'CPU graphics helper', $(& $on ($c.igpu -ne 'off')), 'browsers / Discord / Spotify use the CPU''s own graphics, so the card''s memory is the game''s'),
            @('J', 'Saved game', '', 'continue or new game (from the very beginning), bring a save back, ask before launch on/off'),
            @('P', 'Prepare again', '', 'translate every shader and make every pipeline again with the game closed'),
            @('S', 'Reports folder', '', "where report zips are saved: $(Reports-Dir)"),
            @('E', 'Open reports', '', 'open the reports folder in Explorer'),
            @('Q', 'Quit', '', 'close VIctory Try')
        )
        Write-Host ''
        Write-Host '  ================= COMMANDS, AvatarManz engine (type the key, then Enter) =================' -ForegroundColor Cyan
        Write-Host ('  {0,-6} {1,-18} {2,-4}  {3}' -f 'KEY', 'COMMAND', 'NOW', 'WHAT IT DOES') -ForegroundColor Cyan
        foreach ($r in $rows) { Write-Host ('  {0,-6} {1,-18} {2,-4}  {3}' -f $r[0], $r[1], $r[2], $r[3]) }
        Write-Host '  ===========================================================================================' -ForegroundColor Cyan
        Write-Host '  Switches (N, C, R, Y, B, X, A, F, Z, I) flip at once and are saved; Enter then runs the game with them.'
        return
    }
    $rows = @(
        @('Enter', 'Run again', '', 'start the game again with the settings below'),
        @('U', 'Update', '', 'download the newest VIctory, build it, then run the game'),
        @('N', 'Engine', $(Engine-Name), 'own: this fork''s (default, the others'' fixes ported in); avm: AvatarManz/Belobug''s; 8GB: soldsawn''s'),
        @('V', 'Version', '', 'show which Try and VIctory version you have'),
        @('G', 'Game paths', '', 'ask again where the game folder and game_scarlett_master.exe are'),
        @('C', 'Console', $(if ($c.console -eq 'series-s') { 'S  ' } else { 'X  ' }), 'Xbox Series S (renders 1280x720 and scales up: faster) or Series X (full size)'),
        @('R', 'Screen size', $(if ($c.res -eq '4k') { '4K ' } else { '1080' }), 'the size the game is told the screen has: 1920x1080 or 3840x2160'),
        @('F', 'Speed mode', $(& $on ($c.fast -eq '1')), 'faster copying between PC and card; riskier (back to stable if the card is lost)'),
        @('B', 'Fps boost', $(& $on ($c.boost -ne 'off')), 'Series X: renders at 75% and sharpens; both: skips ray tracing (gave black shadows)'),
        @('M', 'Frames in between', $(& $on ($c.between -eq '1')), 'adds a moved frame between the game''s own (smoother; Series X only)'),
        @('H', 'Shared buffers', $(& $on ($c.hybrid -eq '1')), 'small buffers shared with the card instead of copied'),
        @('A', 'Ambient light', $(switch ($c.ambient) { 'off' { 'off' } 'low' { 'low' } 'high' { 'hi ' } default { 'ON ' } }), 'soft light where the sun does not reach: normal, low (if people look too bright), high, off'),
        @('L', 'Textures by level', $(& $on ($c.levels -ne 'off')), 'textures take card memory only for the sizes in use (8 GB cards: no black doors)'),
        @('O', 'Shader clean-up', $(& $on ($c.opt -eq 'on')), 'off keeps walls, roofs and floors (they went missing with it on)'),
        @('W', 'Shadow fix', $(& $on ($c.shadowfix -ne 'off')), 'NVIDIA: keeps the ceiling and lit rooms (14 shadow programs translated safely)'),
        @('I', 'CPU graphics helper', $(& $on ($c.igpu -ne 'off')), 'browsers / Discord / Spotify use the CPU''s own graphics, so the card''s memory is the game''s'),
        @('K', 'Copies on the card', $(& $on ($c.gpucopy -ne 'off')), 'faster: the game''s buffer copies run on the card (no waiting for it); off if things look wrong'),
        @('T', 'Upscaler', $(switch ($c.temporal) { 'off' { 'off' } 'own' { 'own' } default { 'FSR' } }), 'FSR: AMD''s PC FidelityFX upscaler (sharp, right reflections); own: VIctory''s frame accumulation; off: plain stretch'),
        @('Y', 'fxceless speed set', $(& $on ($c.fxspeed -ne 'off')), 'heap work on its own thread, texture checks side by side, settle room (fxceless/victory-fork)'),
        @('X', 'Low preset', $(& $on $script:LowFromTry), '6 GB cards: game told 1280x720, textures up to 1536 MB (faster, blurrier; on by itself under 7 GB)'),
        @('J', 'Saved game', '', 'continue or new game (from the very beginning), bring a save back, ask before launch on/off'),
        @('D', 'Recommended', '', 'every switch back to the recommended settings (renders the most)'),
        @('MAX', 'All-out mode', $(& $on (AllOut)), 'no caps: every CPU thread but one, high priority, textures up to 99% of the card, memory guard at 1% free (risky)'),
        @('Z', 'Plain mode', $(& $on ($c.plain -eq '1')), 'VIctory''s own defaults instead of Try''s settings (for comparing)'),
        @('P', 'Prepare again', '', 'compile all pipelines again with the game closed (done by itself after updates)'),
        @('TIDY', 'Disk', '', 'what VIctory Try takes on the disk, and old builds, logs and other engines removed'),
        @('S', 'Reports folder', '', "where report zips are saved: $(Reports-Dir)"),
        @('E', 'Open reports', '', 'open the reports folder in Explorer'),
        @('Q', 'Quit', '', 'close VIctory Try')
    )
    Write-Host ''
    Write-Host '  ======================= COMMANDS (type the key, then Enter) =======================' -ForegroundColor Cyan
    Write-Host ('  {0,-6} {1,-18} {2,-4}  {3}' -f 'KEY', 'COMMAND', 'NOW', 'WHAT IT DOES') -ForegroundColor Cyan
    foreach ($r in $rows) {
        Write-Host ('  {0,-6} {1,-18} {2,-4}  {3}' -f $r[0], $r[1], $r[2], $r[3])
    }
    Write-Host '  ====================================================================================' -ForegroundColor Cyan
    Write-Host '  Switches (N, C, R, F, B, M, H, A, L, O, W, K, T, Y, X, Z, D, MAX) flip at once and are saved; Enter then runs the game with them.'
}

function Show-Version {
    Step 'Version'
    Say "VIctory Try $TryVersion"
    $c = $script:Config
    if ("$($c.built_key)" -match '^fork:([0-9a-f]{7,40})') {
        $line = (G log -1 --format='%h %ad %s' --date=short $Matches[1]) | Select-Object -First 1
        Say "VIctory built from: $line"
    } else {
        Say 'VIctory: not built yet'
    }
    $fetched = (G rev-parse -q --verify refs/remotes/origin/fork) | Select-Object -First 1
    if ($fetched) {
        $line = (G log -1 --format='%h %ad %s' --date=short $fetched) | Select-Object -First 1
        Say "newest downloaded: $line   (U updates)"
    }
    Say "game: $($c.game_exe)"
}

while ($true) {
    Run-Game
    Show-Commands
    # Switches only change the setting and show the list again; Enter (or
    # U, G, P) runs the game, Q quits.
    $k = ''
    while ($true) {
        $k = (Read-Host '  Choice').Trim().ToUpper()
        if ($k -eq 'V') { Show-Version; Show-Commands; continue }
        if ($k -eq 'TIDY') { Tidy-Disk $true; Show-Commands; continue }
        if ($k -eq 'J') { Save-Menu; Show-Commands; continue }
        if ($k -eq 'S') { Ask-Reports; Save-Config; Show-Commands; continue }
        if ($k -eq 'E') { try { Start-Process explorer.exe -ArgumentList "`"$(Reports-Dir)`"" } catch { Bad "could not open it: $(Reports-Dir)" }; Show-Commands; continue }
        $c = $script:Config
        $switched = $true
        if ((EngineAvm) -and @('F', 'Z', 'B', 'Y', 'X') -contains $k) {
            switch ($k) {
                'F' { $c.unlock = $(if ($c.unlock -eq '1') { '' } else { '1' }) }
                'Z' { $c.safe = $(if ($c.safe -eq '1') { '' } else { '1' }) }
                'B' { $c.rt = $(if ($c.rt -eq '1') { '' } else { '1' }) }
                'Y' { $c.ports = $(if ($c.ports -eq 'off') { '' } else { 'off' }) }
                'X' { $c.texcap = $(switch ($c.texcap) { 'sharp' { 'low' } 'low' { '' } default { 'sharp' } }) }
            }
            Save-Config
            Say "switched ($k). Press another switch, or Enter to run the game."
            Show-Commands
            continue
        }
        switch ($k) {
            'F' { $c.fast = $(if ($c.fast -eq '1') { '' } else { '1' }) }
            'Z' { $c.plain = $(if ($c.plain -eq '1') { '' } else { '1' }) }
            'B' { $c.boost = $(if ($c.boost -eq 'off') { '' } else { 'off' }) }
            'M' { $c.between = $(if ($c.between -eq '1') { '' } else { '1' }) }
            'H' { $c.hybrid = $(if ($c.hybrid -eq '1') { '' } else { '1' }) }
            'A' { $c.ambient = $(switch ($c.ambient) { 'low' { 'high' } 'high' { 'off' } 'off' { '' } default { 'low' } }) }
            'L' { $c.levels = $(if ($c.levels -eq 'off') { '' } else { 'off' }) }
            'O' { $c.opt = $(if ($c.opt -eq 'on') { '' } else { 'on' }) }
            'W' { $c.shadowfix = $(if ($c.shadowfix -eq 'off') { '' } else { 'off' }) }
            'K' { $c.gpucopy = $(if ($c.gpucopy -eq 'off') { '' } else { 'off' }) }
            'T' { $script:FsrRetry = $true; $c.temporal = $(switch ($c.temporal) { 'own' { 'off' } 'off' { '' } default { 'own' } }) }
            'Y' { $c.fxspeed = $(if ($c.fxspeed -eq 'off') { '' } else { 'off' }) }
            'X' { $c.low = $(if ($script:LowFromTry) { 'off' } else { '1' }) }
            'I' { if ($c.igpu -eq 'off') { $c.igpu = '' } else { $c.igpu = 'off'; Undo-Helper } }
            'R' { $c.res = $(if ($c.res -eq '4k') { '' } else { '4k' }) }
            'C' { $c.console = $(if ($c.console -eq 'series-s') { 'series-x' } else { 'series-s' }); $c.console_auto = '5.6'; $c.console_user = '1' }
            'D' { Reset-Recommended; $c = $script:Config }
            'MAX' { $c.allout = $(if ($c.allout -eq '1') { '' } else { '1' }) }
            'N' { $c.engine = $(switch ($c.engine) { 'avm' { '8gb' } '8gb' { 'own' } default { 'avm' } }); $null = Ensure-Engine $false }
            default { $switched = $false }
        }
        if ($switched) {
            Save-Config
            Say "switched ($k). Press another switch, or Enter to run the game."
            Show-Commands
            continue
        }
        if ($k -eq '' -or $k -eq 'U' -or $k -eq 'G' -or $k -eq 'P' -or $k -eq 'Q') { break }
        Bad "unknown key '$k': use one of the keys in the list"
        Show-Commands
    }
    if ($k -eq 'Q') { exit 0 }
    if ($k -eq 'G') { while (-not (Ask-Game $true)) {} }
    if ($k -eq 'P') { $script:Config.prepared = ''; $script:Config.recipes = ''; $script:Config.shaders_all = ''; $script:Config.avm_shaders = ''; $script:Config.avm_warm = '' }
    if ($k -eq 'U') {
        $script:FsrRetry = $true
        $changed = Update-Source
        # 5.7: a newer VIctory Try takes over at once (its own fixes, not
        # only the game's, without closing this window first).
        Save-Config; Run-Newer -FromMenu
        # (Run-Newer did not hand over: this is the newest VIctory Try.)
        Say "VIctory Try $TryVersion is the newest one on the server"
        if (-not (OwnEngine)) { $null = Ensure-Engine $true }
        elseif ($changed -or -not (Test-Path (Ours-Exe))) { if (-not (Build-All)) { Bad 'the new build FAILED: the game runs on the last working build (older fixes). Send the error above.' } }
    }
}
