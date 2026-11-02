# Dot-source me from any Windows-agent pipeline step:  . .\scripts\ci\windows-env.ps1
#
# The Windows counterpart of macos-env.sh, and it exists for the same reason: the Windows
# agent runs Woodpecker's LOCAL backend, so a step is a process on the host rather than a
# container, and it inherits the agent service's environment. That environment is not a
# login shell's:
#
#   * the toolchain is not on PATH. rustup installs into a user profile's `.cargo\bin`,
#     and the agent runs as a service account whose profile has neither;
#   * CARGO_HOME and RUSTUP_HOME are therefore wrong too. rustup reads its default
#     toolchain from RUSTUP_HOME, and with the service account's empty profile `cargo`
#     dies with "could not choose a version of cargo to run" — a message that reads as a
#     broken toolchain rather than as a missing variable (this is what ttr3's Windows leg
#     hit);
#   * there is no bash, and every shared script in scripts/ci is sh. Git for Windows
#     ships one, so this finds it and exports CI_BASH rather than having each step guess.
#
# Exports: PATH (with cargo on it), CARGO_HOME, RUSTUP_HOME, CI_BASH, CACHE.
#
# CACHE and anything else a step hands to a scripts/ci/*.sh use FORWARD SLASHES. These
# paths cross into Git Bash, where a backslash is an escape character, so `mkdir -p
# "C:\cache\target"` silently makes a directory called `C:cachetarget`. Cargo and every
# Windows API take forward slashes, so one spelling works on both sides.

$ErrorActionPreference = 'Stop'

# ---- the Rust toolchain ------------------------------------------------------
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    # C:\ci-tools first, where this fleet's Windows agent keeps hand-installed tools,
    # then any user profile that has a cargo.exe.
    $candidates = @('C:\ci-tools\.cargo\bin')
    $candidates += Get-ChildItem C:\Users -Directory -ErrorAction SilentlyContinue |
        ForEach-Object { Join-Path $_.FullName '.cargo\bin' }
    $cargoBin = $candidates | Where-Object { Test-Path (Join-Path $_ 'cargo.exe') } | Select-Object -First 1
    if (-not $cargoBin) {
        throw 'windows-env: no cargo.exe under C:\ci-tools or any profile in C:\Users — this agent has no Rust toolchain'
    }
    $env:Path = $cargoBin + ';' + $env:Path
    $profileRoot = Split-Path (Split-Path $cargoBin -Parent) -Parent
    $env:CARGO_HOME = Join-Path $profileRoot '.cargo'
    $env:RUSTUP_HOME = Join-Path $profileRoot '.rustup'
}
Write-Host ('windows-env: cargo ' + ((cargo --version) -join ' '))

# ---- Git Bash ----------------------------------------------------------------
# Beside the git on PATH, then the default install. Without it no shared script runs.
$bash = $null
$git = (Get-Command git -ErrorAction SilentlyContinue).Source
if ($git) {
    $gitRoot = Split-Path (Split-Path $git -Parent) -Parent
    foreach ($c in @((Join-Path $gitRoot 'bin\bash.exe'), (Join-Path $gitRoot 'usr\bin\bash.exe'))) {
        if (Test-Path $c) { $bash = $c; break }
    }
}
if (-not $bash -and (Test-Path 'C:\Program Files\Git\bin\bash.exe')) { $bash = 'C:\Program Files\Git\bin\bash.exe' }
if (-not $bash) { throw 'windows-env: no Git Bash on this agent — scripts/ci/*.sh cannot run' }
$env:CI_BASH = $bash

# ---- jq ----------------------------------------------------------------------
# publish.sh needs it, and the registry client dies with a message that says neither
# where it is missing nor how to get it. There is no package manager to lean on here, so
# this only reports: installing it is an agent chore.
& $env:CI_BASH -c 'command -v jq >/dev/null' | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw 'windows-env: no jq on this agent — install it (scoop/choco, or drop jq.exe in C:\ci-tools) and re-run'
}

# ---- the cache root ----------------------------------------------------------
# Outside the workspace, which Woodpecker hands out fresh per run, and under the service
# account's own local app data rather than a throwaway HOME.
$appData = [Environment]::GetFolderPath('LocalApplicationData')
if (-not $appData) { $appData = $env:LOCALAPPDATA }
if (-not $appData) { $appData = 'C:\ProgramData' }
$cache = Join-Path $appData 'woodpecker\standards'
New-Item -ItemType Directory -Force -Path $cache | Out-Null
$env:CACHE = $cache.Replace('\', '/')
Write-Host ('windows-env: CACHE ' + $env:CACHE)
