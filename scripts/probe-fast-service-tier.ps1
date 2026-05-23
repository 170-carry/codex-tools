[CmdletBinding()]
param(
    [string]$BaseUrl = 'http://127.0.0.1:8666/v1',
    [string]$ApiKeyPath = 'F:\Codex Tools\Codex Tools Data\api-proxy.key',
    [string]$Model = 'gpt-5.5',
    [string]$ServiceTier = 'priority'
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path -LiteralPath $ApiKeyPath)) {
    throw "API key file not found: $ApiKeyPath"
}

$apiKey = (Get-Content -LiteralPath $ApiKeyPath -Raw).Trim()
$payload = [ordered]@{
    model = $Model
    input = @(
        @{
            role = 'user'
            content = @(
                @{
                    type = 'input_text'
                    text = 'Reply with exactly OK.'
                }
            )
        }
    )
    reasoning = @{ effort = 'low'; summary = 'auto' }
    service_tier = $ServiceTier
    stream = $false
    store = $false
}

$bodyPath = Join-Path $env:TEMP 'codex-tools-fast-tier-probe.json'
$responsePath = Join-Path $env:TEMP 'codex-tools-fast-tier-probe-response.json'
$bodyJson = $payload | ConvertTo-Json -Depth 12
[System.IO.File]::WriteAllText($bodyPath, $bodyJson, [System.Text.UTF8Encoding]::new($false))

$httpStatus = & curl.exe -sS -o $responsePath -w '%{http_code}' -H "Authorization: Bearer $apiKey" -H 'Content-Type: application/json' --data-binary "@$bodyPath" "$BaseUrl/responses"
$response = Get-Content -LiteralPath $responsePath -Raw
$json = $response | ConvertFrom-Json
$outputText = if ($json.output_text -is [array]) { $json.output_text -join '' } else { $json.output_text }

[pscustomobject]@{
    http_status = [int]$httpStatus
    requested_service_tier = $ServiceTier
    actual_service_tier = $json.service_tier
    model = $json.model
    status = $json.status
    error = $json.error.message
    output_text = $outputText
} | ConvertTo-Json -Depth 4
