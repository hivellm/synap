# Script to reformat CHANGELOG.md to match Vectorizer format
# Removes emojis, checkmarks, and excessive formatting

$changelogPath = "CHANGELOG.md"
$content = Get-Content $changelogPath -Raw -Encoding UTF8

# Remove emojis and checkmarks
$content = $content -replace '✅', ''
$content = $content -replace '🚀', ''
$content = $content -replace '🎉', ''
$content = $content -replace '⬆️', ''
$content = $content -replace '📄', ''
$content = $content -replace '🆕', ''
$content = $content -replace '🔧', ''
$content = $content -replace '🐛', ''
$content = $content -replace '🗑️', ''
$content = $content -replace '🔥', ''
$content = $content -replace '📝', ''
$content = $content -replace '🔒', ''
$content = $content -replace '⏳', ''
$content = $content -replace '🔄', ''
$content = $content -replace '💡', ''
$content = $content -replace '🎯', ''
$content = $content -replace '📊', ''
$content = $content -replace '⚡', ''
$content = $content -replace '✨', ''
$content = $content -replace '🎨', ''
$content = $content -replace '🔍', ''
$content = $content -replace '📦', ''
$content = $content -replace '🏗️', ''
$content = $content -replace '🧪', ''
$content = $content -replace '📈', ''
$content = $content -replace '🔐', ''
$content = $content -replace '🌐', ''
$content = $content -replace '💻', ''
$content = $content -replace '🎁', ''
$content = $content -replace '🌟', ''
$content = $content -replace '🔨', ''
$content = $content -replace '📋', ''
$content = $content -replace '🎪', ''
$content = $content -replace '🎬', ''
$content = $content -replace '🎭', ''
$content = $content -replace '🎨', ''
$content = $content -replace '🎯', ''
$content = $content -replace '🎲', ''
$content = $content -replace '🎸', ''
$content = $content -replace '🎺', ''
$content = $content -replace '🎻', ''
$content = $content -replace '🎼', ''
$content = $content -replace '🎵', ''
$content = $content -replace '🎶', ''
$content = $content -replace '🎤', ''
$content = $content -replace '🎧', ''
$content = $content -replace '🎬', ''
$content = $content -replace '🎞️', ''
$content = $content -replace '🎟️', ''
$content = $content -replace '🎫', ''
$content = $content -replace '🎪', ''
$content = $content -replace '🎭', ''
$content = $content -replace '🎨', ''
$content = $content -replace '🎯', ''
$content = $content -replace '🎲', ''
$content = $content -replace '🎮', ''
$content = $content -replace '🎰', ''
$content = $content -replace '🎱', ''
$content = $content -replace '🎳', ''
$content = $content -replace '🎴', ''
$content = $content -replace '🎵', ''
$content = $content -replace '🎶', ''
$content = $content -replace '🎤', ''
$content = $content -replace '🎧', ''
$content = $content -replace '🎬', ''
$content = $content -replace '🎞️', ''
$content = $content -replace '🎟️', ''
$content = $content -replace '🎫', ''
$content = $content -replace '🎪', ''
$content = $content -replace '🎭', ''
$content = $content -replace '🎨', ''
$content = $content -replace '🎯', ''
$content = $content -replace '🎲', ''
$content = $content -replace '🎮', ''
$content = $content -replace '🎰', ''
$content = $content -replace '🎱', ''
$content = $content -replace '🎳', ''
$content = $content -replace '🎴', ''

# Remove "### Added -" and "### Fixed -" patterns, keep just "### Added" or "### Fixed"
$content = $content -replace '### Added - ', "### Added`n`n"
$content = $content -replace '### Fixed - ', "### Fixed`n`n"
$content = $content -replace '### Changed - ', "### Changed`n`n"

# Remove bold markers from list items but keep structure
$content = $content -replace '- \*\*([^*]+)\*\* - ', '- $1: '

# Clean up multiple blank lines
$content = $content -replace "`r?`n`r?`n`r?`n+", "`r`n`r`n"

# Remove "**Implementation Status**" sections and similar verbose sections
$content = $content -replace '(?s)\*\*Implementation Status\*\*.*?\*\*Migration Path\*\*', ''

# Remove "**Breaking Changes**" sections (keep content but simplify)
$content = $content -replace '(?s)\*\*Breaking Changes\*\*', '### Breaking Changes'

# Remove "**Migration Path**" sections
$content = $content -replace '(?s)\*\*Migration Path\*\*.*?(?=##|$)', ''

# Remove "**Files Changed**" sections
$content = $content -replace '(?s)\*\*Files Changed\*\*:.*?(?=\*\*|##|$)', ''

# Remove "**Test Coverage**" sections
$content = $content -replace '(?s)\*\*Test Coverage\*\*:.*?(?=\*\*|##|$)', ''

# Remove "**Impact**" sections
$content = $content -replace '(?s)\*\*Impact\*\*:.*?(?=\*\*|##|$)', ''

# Remove "**Migration Notes**" sections
$content = $content -replace '(?s)\*\*Migration Notes\*\*:.*?(?=\*\*|##|$)', ''

# Clean up again
$content = $content -replace "`r?`n`r?`n`r?`n+", "`r`n`r`n"

# Write back
Set-Content -Path $changelogPath -Value $content -Encoding UTF8 -NoNewline

Write-Host "CHANGELOG.md reformatted successfully!"

