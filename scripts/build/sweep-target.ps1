<#
.SYNOPSIS
  Bound the Cargo `target/` directory size (Windows / PowerShell).

.DESCRIPTION
  Cargo never garbage-collects target/: stale object files, incremental caches,
  and rlibs from old dependency versions accumulate until something deletes them.
  This wraps `cargo-sweep` to remove artifacts not accessed in N days WITHOUT
  breaking incrementality — the hot set (recently touched artifacts) stays.

  See docs/rust-target-hygiene.md for the full rationale.

.PARAMETER Days
  Remove artifacts not accessed in the last N days (default 14).

.PARAMETER Clean
  Run a full `cargo clean` (reclaim everything) instead of a sweep.

.PARAMETER DryRun
  Show what would be removed without deleting anything.

.EXAMPLE
  scripts/build/sweep-target.ps1              # sweep artifacts unused for 14 days
  scripts/build/sweep-target.ps1 -Days 30
  scripts/build/sweep-target.ps1 -Clean
  scripts/build/sweep-target.ps1 -DryRun
#>
[CmdletBinding()]
param(
  [int]$Days = 14,
  [switch]$Clean,
  [switch]$DryRun
)

$ErrorActionPreference = 'Stop'

# Resolve repo root (one dir up from this script) so the sweep works from anywhere.
$RepoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $RepoRoot

function Get-TargetSize {
  if (Test-Path 'target') {
    $bytes = (Get-ChildItem 'target' -Recurse -File -ErrorAction SilentlyContinue |
      Measure-Object -Property Length -Sum).Sum
    if (-not $bytes) { return '0 B' }
    return '{0:N2} GB' -f ($bytes / 1GB)
  }
  return '0 B'
}

Write-Host "target/ size before: $(Get-TargetSize)"

if ($Clean) {
  Write-Host "Running full 'cargo clean'..."
  cargo clean
  Write-Host "target/ size after:  $(Get-TargetSize)"
  exit 0
}

# Ensure cargo-sweep is available; install on first use.
& cargo sweep --version *> $null
if ($LASTEXITCODE -ne 0) {
  Write-Host "cargo-sweep not found - installing (cargo install cargo-sweep)..."
  cargo install cargo-sweep --locked
}

if ($DryRun) {
  Write-Host "Dry run: artifacts unused for > $Days days (nothing will be deleted)..."
  cargo sweep --dry-run --time $Days
} else {
  Write-Host "Sweeping artifacts unused for > $Days days..."
  cargo sweep --time $Days
}

Write-Host "target/ size after:  $(Get-TargetSize)"
