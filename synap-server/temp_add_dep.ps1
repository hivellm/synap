$content = Get-Content Cargo.toml -Raw
$find = 'geohash = "0.13"'
$replace = @"
geohash = "0.13"

# HiveHub Cloud Integration
hivehub-internal-sdk = { path = "../../hivehub-cloud/sdks/internal-sdk", optional = true }
"@
$newContent = $content.Replace($find, $replace)
Set-Content -Path Cargo.toml -Value $newContent -NoNewline
