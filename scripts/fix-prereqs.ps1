<#
.SYNOPSIS
  One-time fixes BudMix needs admin rights for. BudMix runs this script
  elevated (one UAC prompt) only for the tasks the user clicked.

.PARAMETER Tasks
  Comma-separated: firewall, wirelessdisplay.
.PARAMETER Program
  Full path of BudMix.exe (task "firewall").
#>
param(
    [string]$Tasks = "",
    [string]$Program = ""
)

$ErrorActionPreference = "Stop"
$log = Join-Path $env:TEMP "budmix-fix.log"
$failed = 0

function Write-Log([string]$message) {
    $line = "{0:u}  {1}" -f (Get-Date), $message
    Add-Content -Path $log -Value $line
    Write-Host $line
}

foreach ($task in $Tasks.Split(",") | ForEach-Object { $_.Trim().ToLowerInvariant() } | Where-Object { $_ }) {
    try {
        switch ($task) {
            "firewall" {
                if (-not (Test-Path -LiteralPath $Program)) { throw "program not found: $Program" }
                # "Cancel" on Windows' own firewall prompt leaves Block rules, and
                # a Block rule beats every Allow rule.
                Get-NetFirewallApplicationFilter -Program $Program -ErrorAction SilentlyContinue |
                    Get-NetFirewallRule |
                    Where-Object { $_.Direction -eq "Inbound" -and $_.Action -eq "Block" } |
                    Remove-NetFirewallRule
                foreach ($name in "BudMix (Private networks)", "BudMix (phone cast link)") {
                    Get-NetFirewallRule -DisplayName $name -ErrorAction SilentlyContinue |
                        Remove-NetFirewallRule
                }
                New-NetFirewallRule -DisplayName "BudMix (Private networks)" `
                    -Description "Lets a phone's Miracast cast reach BudMix on Private networks." `
                    -Direction Inbound -Action Allow -Profile Private -Program $Program | Out-Null
                # Windows treats the direct Wi-Fi link to the phone as Public.
                New-NetFirewallRule -DisplayName "BudMix (phone cast link)" `
                    -Description "Lets the sound of a casting phone reach BudMix over the direct Wi-Fi link." `
                    -Direction Inbound -Action Allow -Profile Public -RemoteAddress LocalSubnet `
                    -Program $Program | Out-Null
                Write-Log "firewall: rules added for $Program"
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
