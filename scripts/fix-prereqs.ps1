<#
.SYNOPSIS
  One-time fixes BudsLink needs admin rights for. BudsLink runs this script
  elevated (one UAC prompt) only for the tasks the user clicked.

.PARAMETER Tasks
  Comma-separated: network, firewall, wirelessdisplay.
.PARAMETER Network
  Name of the Wi-Fi network to mark Private (task "network").
.PARAMETER Program
  Full path of BudsLink.exe (task "firewall").
#>
param(
    [string]$Tasks = "",
    [string]$Network = "",
    [string]$Program = ""
)

$ErrorActionPreference = "Stop"
$log = Join-Path $env:TEMP "budslink-fix.log"
$failed = 0

function Write-Log([string]$message) {
    $line = "{0:u}  {1}" -f (Get-Date), $message
    Add-Content -Path $log -Value $line
    Write-Host $line
}

foreach ($task in $Tasks.Split(",") | ForEach-Object { $_.Trim().ToLowerInvariant() } | Where-Object { $_ }) {
    try {
        switch ($task) {
            "network" {
                if (-not $Network) { throw "no network name given" }
                # Only the network the user confirmed, never every Public one.
                $profiles = @(Get-NetConnectionProfile | Where-Object { $_.Name -eq $Network })
                if ($profiles.Count -eq 0) { throw "network '$Network' is not connected" }
                foreach ($profile in $profiles) {
                    Set-NetConnectionProfile -InterfaceIndex $profile.InterfaceIndex -NetworkCategory Private
                }
                Write-Log "network: '$Network' is now Private"
            }
            "firewall" {
                if (-not (Test-Path -LiteralPath $Program)) { throw "program not found: $Program" }
                Get-NetFirewallRule -DisplayName "BudsLink (Private networks)" -ErrorAction SilentlyContinue |
                    Remove-NetFirewallRule
                New-NetFirewallRule -DisplayName "BudsLink (Private networks)" `
                    -Description "Lets a phone's Miracast cast reach BudsLink on Private networks only." `
                    -Direction Inbound -Action Allow -Profile Private -Program $Program | Out-Null
                Write-Log "firewall: rule added for $Program (Private only)"
            }
            "wirelessdisplay" {
                Add-WindowsCapability -Online -Name "App.WirelessDisplay.Connect~~~~0.0.1.0" | Out-Null
                Write-Log "wirelessdisplay: installed"
            }
            default { throw "unknown task '$task'" }
        }
    } catch {
        $failed++
        Write-Log "${task}: FAILED: $($_.Exception.Message)"
    }
}

exit $failed
