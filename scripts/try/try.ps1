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
$TryVersion = '4.5'
$ForkUrl = 'https://github.com/MfDev0/victory.git'
$Branch = 'arena/59fabb00-victory'
# Other places the fork may live (renamed to CAtez, or a backup copy), tried
# when the fork itself cannot be reached.
$MirrorUrls = @('https://github.com/MfDev0/CAtez.git', 'https://github.com/MfDev0/CAtez-backup.git')
$UpstreamUrl = 'https://github.com/Marvisak/VIctory.git'
$MinGitUrl = 'https://github.com/git-for-windows/git/releases/download/v2.56.1.windows.1/MinGit-2.56.1-64-bit.zip'
$TryHome = Join-Path $env:LOCALAPPDATA 'VIctoryTry'
$Src = Join-Path $TryHome 'victory'
$Target = Join-Path $TryHome 'target'
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
    $c = @{ game_dir = ''; game_exe = ''; built_key = ''; added = ''; console = 'series-x'; base = ''; fast = ''; loop = ''; prepared = ''; recipes = ''; wide = ''; wavewide = ''; new38 = ''; lazy = ''; res = ''; plain = ''; boost = ''; between = ''; hybrid = ''; ambient = ''; levels = ''; opt = ''; shadowfix = '' }
    if (Test-Path $ConfigPath) {
        try {
            $j = Get-Content -Path $ConfigPath -Raw | ConvertFrom-Json
            foreach ($k in @('game_dir', 'game_exe', 'built_key', 'added', 'console', 'base', 'fast', 'loop', 'prepared', 'recipes', 'wide', 'wavewide', 'new38', 'lazy', 'res', 'plain', 'boost', 'between', 'hybrid', 'ambient', 'levels', 'opt', 'shadowfix')) { if ($j.$k) { $c[$k] = [string]$j.$k } }
        } catch {}
    }
    # Try 3.0 moved to the victory-gta6 base: start once on Series X.
    if ($c.base -ne 'gta6') { $c.console = 'series-x'; $c.base = 'gta6' }
    return $c
}
function Save-Config { try { ($script:Config | ConvertTo-Json) | Set-Content -Path $ConfigPath -Encoding ASCII } catch {} }

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
    if (($key -eq $script:Config.built_key) -and (Test-Path (Victory-Exe))) {
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
    Get-ChildItem -Path $BackupDir -Filter 'catez-*.bundle' | Sort-Object LastWriteTime -Descending |
        Select-Object -Skip 5 | Remove-Item -Force -ErrorAction SilentlyContinue
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

function Victory-Exe { return Join-Path $Target 'release\victory.exe' }

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
    Push-Location $Src
    try {
        # Out-Host: the lines must not become this function's result (they
        # did up to Try 4.0, so a failed build counted as built and the old
        # victory.exe ran on).
        & cargo build --release -p victory 2>&1 | ForEach-Object { "$_" } | Tee-Object -FilePath $log | Out-Host
        $code = $LASTEXITCODE
    } finally { Pop-Location }
    if ($code -ne 0 -or -not (Test-Path (Victory-Exe))) {
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
function Build-All {
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

# ------------------------------------------------------------- run

# The settings a run uses, also for Prepare-Shaders (the shader cache keys
# its translations by some of them: the same ones are needed both times).
function Set-RunEnv {
    $c = $script:Config
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
    if (-not $script:NewFromTry) {
        foreach ($k in @('VICTORY_STAND_IN', 'VICTORY_GPU_UPLOAD', 'VICTORY_PC_CARD_MB', 'VICTORY_COPY_FETCH', 'VICTORY_TRIM_BARRIERS')) { [Environment]::SetEnvironmentVariable($k, '0', 'Process') }
    }
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
        foreach ($k in @('VICTORY_SHADER_LOOP_LIMIT', 'VICTORY_LATE_WAIT', 'VICTORY_WIDE_BACK', 'VICTORY_CARD_BUFFERS_MIN_MB', 'VICTORY_CARD_BUFFERS_MB', 'VICTORY_RELAY', 'VICTORY_WAVE_WIDE', 'VICTORY_LAZY_BACK', 'VICTORY_STAND_IN', 'VICTORY_GPU_UPLOAD', 'VICTORY_PC_CARD_MB', 'VICTORY_COPY_FETCH', 'VICTORY_TRIM_BARRIERS')) {
            [Environment]::SetEnvironmentVariable($k, $null, 'Process')
        }
        $script:LoopFromTry = $false; $script:WideFromTry = $false; $script:WaveWideFromTry = $false
        $script:NewFromTry = $false; $script:LazyFromTry = $false
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
                'VICTORY_FRAMEGEN' { $c.between -eq '1' }
                'VICTORY_INPLACE' { $c.hybrid -eq '1' }
                default { $c.boost -ne 'off' }
            }
            [Environment]::SetEnvironmentVariable($name, $(if ($want) { $kv[1] } else { $null }), 'Process')
        }
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
    if ($null -eq $script:UserFx['VICTORY_AMBIENT']) { $env:VICTORY_AMBIENT = $(if ($c.ambient -eq 'off') { 'off' } else { '0.6' }) }
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
    if ($null -eq $script:UserFx['VICTORY_SHADER_OPT']) { [Environment]::SetEnvironmentVariable('VICTORY_SHADER_OPT', $(if ($c.opt -eq 'on') { $null } else { '0' }), 'Process') }
    if ($null -eq $script:UserFx['VICTORY_WAVE_DEEP_SKIPS_NOT']) { [Environment]::SetEnvironmentVariable('VICTORY_WAVE_DEEP_SKIPS_NOT', $(if ($c.shadowfix -eq 'off') { 'none' } else { $null }), 'Process') }
    Say "walls: shader clean-up $(if ($env:VICTORY_SHADER_OPT -eq '0') { 'off (walls and roofs kept)' } else { 'ON' })   (O switches)   shadow fix $(if ($env:VICTORY_WAVE_DEEP_SKIPS_NOT -eq 'none') { 'off' } else { 'ON' })   (W switches)"
    Say "picture: ambient light $(if ($env:VICTORY_AMBIENT -ne 'off') { 'ON' } else { 'off' })   (A switches)   textures level by level $(if ($env:VICTORY_LEVELS_ON_DEMAND -eq '1') { 'ON' } else { 'off' })   (L switches)"
    Say "fps boost: $(if ($c.boost -ne 'off') { 'ON (75% render scale + sharpen, no ray tracing)' } else { 'off' })   frames in between: $(if ($c.between -eq '1') { 'ON' } else { 'off' })   shared small buffers: $(if ($c.hybrid -eq '1') { 'ON' } else { 'off' })"
    # Its own cache: the old base's pipeline library and shaders do not mix.
    if (-not $env:VICTORY_CACHE_DIR) { $env:VICTORY_CACHE_DIR = Join-Path $TryHome 'cache-gta6' }
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
function Prepare-Shaders([string]$why) {
    $c = $script:Config
    Step "Preparing pipelines with the game closed ($why)"
    Say 'Compiles everything the game asked for, so the next launch shows it at once. Can take a few minutes the first time.'
    $log = Join-Path $LogDir 'prepare.log'
    Push-Location $TryHome
    try {
        & (Victory-Exe) prepare 2>&1 | ForEach-Object { "$_" } | Tee-Object -FilePath $log | Where-Object { $_ -match '^(prepar|translated)' }
        $code = $LASTEXITCODE
    } finally { Pop-Location }
    if ($code -eq 0) { Say 'pipelines prepared' }
    else { Bad "preparing stopped (code $code; output in $log): the game makes the rest itself" }
    # Not again for this build and these recipes either way: a failure would repeat each launch.
    $c.prepared = $c.built_key
    $c.recipes = Recipes-Size
    Save-Config
}


function Run-Game {
    $c = $script:Config
    Step 'Running the game with VIctory'
    Say "game: $($c.game_exe)"
    # What victory.exe was built from (not just what was downloaded): a
    # failed build keeps the last working one.
    $built = ''
    if ("$($c.built_key)" -match '^fork:([0-9a-f]{7,40})') { $built = $Matches[1] }
    $fetched = (G rev-parse -q --verify refs/remotes/origin/fork) | Select-Object -First 1
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
    Say "console: Xbox $(if ($c.console -eq 'series-s') { 'Series S' } else { 'Series X' })   (C switches)"
    Say "command: victory.exe run `"$($c.game_exe)`" --execute --console $($c.console)   (VICTORY_GAME_DIR=$($c.game_dir))"
    Say 'To stop: close the game window.'
    Set-RunEnv
    if ("$($c.prepared)" -ne "$($c.built_key)") { Prepare-Shaders 'new build' }
    elseif ("$($c.recipes)" -ne (Recipes-Size)) { Prepare-Shaders 'new pipelines from the last run' }
    # The shared pipeline list (keys of pipelines other players met, no
    # game files): the game starts making them while it loads.
    $shared = Join-Path $Src 'scripts\try\shared-pipelines.txt'
    if (Test-Path -LiteralPath $shared) {
        $pdir = Join-Path $env:VICTORY_CACHE_DIR 'pipelines'
        try {
            New-Item -ItemType Directory -Force -Path $pdir | Out-Null
            Copy-Item -LiteralPath $shared -Destination (Join-Path $pdir 'shared-pipelines.txt') -Force
            $n = @(Get-Content -LiteralPath $shared | Where-Object { $_ -and -not $_.StartsWith('#') }).Count
            Say "shared pipeline list: $n pipelines other players met are made while the game loads"
        } catch { Bad "could not copy the shared pipeline list: $_" }
    }
    Say "failed shaders saved to: $($env:VICTORY_DUMP_FAILED)"
    Say "shader loop limit: $($env:VICTORY_SHADER_LOOP_LIMIT)   skipped programs: $($env:VICTORY_SKIP_PIPELINES)"
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
    Write-Host ''
    Say "the game stopped (exit code $code)"
    Say "its output: $log"
    $crashes = Join-Path $env:LOCALAPPDATA 'victory\data\crashes.log'
    if (Test-Path $crashes) { Say "crash details, if it crashed: $crashes" }
    try { Check-Run $log $stamp $crashes } catch { Bad "could not summarize the run: $_" }
    # Everything this run asked for and did not get, compiled now while the
    # game is closed: the next launch shows it from the start.
    if ("$($c.recipes)" -ne (Recipes-Size)) { Prepare-Shaders 'new pipelines from this run' }
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
        'Write-Host ''== VIctory game output (the menu is in the VIctory Try window)'' -ForegroundColor Cyan',
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
    # Draws still waiting for their pipelines at the end: what is missing
    # (see-through objects) until the next launches have compiled them.
    $lastFrame = @($lines | Where-Object { $_ -match 'dropped ([0-9]+) without a host pipeline \(([0-9]+) being made' }) | Select-Object -Last 1
    if ($lastFrame -and $lastFrame -match 'dropped ([0-9]+) without a host pipeline \(([0-9]+) being made') {
        if ($Matches[2] -eq '0') { Say "at the end: $($Matches[1]) draws a frame skipped (compute work, not objects: mostly the ray-tracing builder, which is off, and the programs counted below)" }
        else { Say "at the end: $($Matches[1]) draws a frame still waiting for $($Matches[2]) pipelines being compiled (fewer each launch: run it again)" }
    }
    Say "shaders that do not translate: $(& $count 'shader does not translate')"
    Say "pipelines the card refused: $(& $count 'host pipeline refused')"
    Say "slow submissions (over 250 ms on the card): $(& $count 'slow host submission')"
    # A crash on a memory access (0xc0000005) with lazy copy-back on: the
    # next run copies back first, as before 3.9.
    $faulted = & $count 'VIctory: .*0xc0000005'
    if ($faulted -gt 0 -and $script:LazyFromTry) {
        $c.lazy = 'off'
        Save-Config
        Bad 'the game crashed on a memory access in this run.'
        Say 'next run: lazy copy-back is off (the card results are copied back before the game hears the card is done).'
    }
    $lost = & $count 'device removed|DEVICE_REMOVED|DEVICE_HUNG'
    if ($lost -gt 0) {
        Bad 'the graphics card was lost (device removed) in this run.'
        if ($c.fast -eq '1') {
            $c.fast = ''
            Say 'next run: back to the stable relay speed mode (F switches again).'
        } elseif ($script:NewFromTry) {
            $c.new38 = 'off'
            Say 'next run: the 3.8 and 3.9 card additions (stand-in pipelines, Resizable BAR uploads, PC-side card copies, copy-engine fetches) are off.'
        } elseif ($script:WaveWideFromTry) {
            $c.wavewide = 'off'
            Say 'next run: the 64-lane compute shaders are left out again, as before 3.6.'
        } elseif ($script:WideFromTry) {
            $c.wide = 'off'
            Say 'next run: copy-back on one thread again, as before 3.5.'
        } elseif ($script:LoopFromTry -and $c.loop -ne '128') {
            $c.loop = '128'
            Say 'next run: shader loop limit 128 instead of 256 (fewer card timeouts; long effects may look a bit off).'
        }
        Save-Config
    }
    # Everything to send in one file on the Desktop.
    $desk = [Environment]::GetFolderPath('Desktop')
    if (-not $desk -or -not (Test-Path $desk)) { $desk = $TryHome }
    $zip = Join-Path $desk "VIctory-report-$stamp.zip"
    $stage = Join-Path $TryHome "report-$stamp"
    try {
        New-Item -ItemType Directory -Force -Path $stage | Out-Null
        Copy-Item -LiteralPath $log -Destination $stage
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
function Run-Newer {
    if ($handoff) { return }
    $text = (G show refs/remotes/origin/fork:scripts/try/try.ps1) -join "`r`n"
    if ($script:GCode -ne 0 -or $text -notmatch "\`$TryVersion = '([0-9.]+)'") { return }
    $theirs = $Matches[1]
    try { if ([version]$theirs -le [version]$TryVersion) { return } } catch { return }
    Step "Starting VIctory Try $theirs (newer than this $TryVersion)"
    $newer = Join-Path $TryHome 'try-latest.ps1'
    Set-Content -LiteralPath $newer -Value $text -Encoding ASCII
    $script:Lock.ReleaseMutex()
    $env:VICTORY_TRY_HANDOFF = '1'
    $ps = (Get-Process -Id $PID).Path
    & $ps -NoProfile -ExecutionPolicy Bypass -File $newer
    exit $LASTEXITCODE
}

$script:Config = Load-Config
$script:UserLoop = $env:VICTORY_SHADER_LOOP_LIMIT
$script:UserWaveWide = $env:VICTORY_WAVE_WIDE
$script:UserDisplay = $env:VICTORY_DISPLAY_MODE
$script:UserFx = @{}
foreach ($name in @('VICTORY_RENDER_SCALE', 'VICTORY_SKIP_RT', 'VICTORY_FRAMEGEN', 'VICTORY_INPLACE', 'VICTORY_AMBIENT', 'VICTORY_LEVELS_ON_DEMAND', 'VICTORY_POOL_NO_ASSERT', 'VICTORY_SHADER_OPT', 'VICTORY_WAVE_DEEP_SKIPS_NOT')) { $script:UserFx[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
while (-not (Ask-Game $false)) {}
if (-not (Ensure-Tools)) { exit 1 }
$changed = Update-Source
if ($null -eq $changed) { Bad 'no source to build: check the internet connection'; exit 1 }
Run-Newer
if ($changed -or -not (Test-Path (Victory-Exe))) { if (-not (Build-All)) { exit 1 } }

while ($true) {
    Run-Game
    Write-Host ''
    Say 'Enter = run again    U = update to the newest version and run'
    Say 'G = change the game paths    C = switch Series S / Series X    F = switch speed mode'
    Say 'R = switch the screen size (1080p / 4K)    Z = switch plain mode (VICTORY defaults, like ztrx)'
    Say 'B = switch fps boost (75% render scale, no ray tracing)    M = switch frames in between    H = switch shared small buffers'
    Say 'A = switch ambient light (no black shadows)    L = switch textures level by level (no black doors and props on 8 GB cards)'
    Say 'O = switch shader clean-up (off keeps walls and roofs)    W = switch the shadow fix (ceiling and lit rooms on NVIDIA)'
    Say 'P = prepare all pipelines again (done by itself after each update and run)    Q = quit'
    $k = (Read-Host '  Choice').Trim().ToUpper()
    if ($k -eq 'Q') { exit 0 }
    if ($k -eq 'G') { while (-not (Ask-Game $true)) {} }
    if ($k -eq 'P') { $script:Config.prepared = ''; $script:Config.recipes = '' }
    if ($k -eq 'F') {
        $script:Config.fast = $(if ($script:Config.fast -eq '1') { '' } else { '1' })
        Save-Config
    }
    if ($k -eq 'Z') {
        $script:Config.plain = $(if ($script:Config.plain -eq '1') { '' } else { '1' })
        Save-Config
    }
    if ($k -eq 'B') {
        $script:Config.boost = $(if ($script:Config.boost -eq 'off') { '' } else { 'off' })
        Save-Config
    }
    if ($k -eq 'M') {
        $script:Config.between = $(if ($script:Config.between -eq '1') { '' } else { '1' })
        Save-Config
    }
    if ($k -eq 'H') {
        $script:Config.hybrid = $(if ($script:Config.hybrid -eq '1') { '' } else { '1' })
        Save-Config
    }
    if ($k -eq 'A') {
        $script:Config.ambient = $(if ($script:Config.ambient -eq 'off') { '' } else { 'off' })
        Save-Config
    }
    if ($k -eq 'L') {
        $script:Config.levels = $(if ($script:Config.levels -eq 'off') { '' } else { 'off' })
        Save-Config
    }
    if ($k -eq 'O') {
        $script:Config.opt = $(if ($script:Config.opt -eq 'on') { '' } else { 'on' })
        Save-Config
    }
    if ($k -eq 'W') {
        $script:Config.shadowfix = $(if ($script:Config.shadowfix -eq 'off') { '' } else { 'off' })
        Save-Config
    }
    if ($k -eq 'R') {
        $script:Config.res = $(if ($script:Config.res -eq '4k') { '' } else { '4k' })
        Save-Config
    }
    if ($k -eq 'C') {
        $script:Config.console = $(if ($script:Config.console -eq 'series-s') { 'series-x' } else { 'series-s' })
        Save-Config
    }
    if ($k -eq 'U') {
        $changed = Update-Source
        if ($changed -or -not (Test-Path (Victory-Exe))) { if (-not (Build-All)) { Bad 'the new build FAILED: the game runs on the last working build (older fixes). Send the error above.' } }
    }
}
