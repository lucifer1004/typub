#!/usr/bin/env bash
# Build mdbook from governance artifacts
# Usage: ./scripts/build-book.sh [--serve] [--skip-render]

set -euo pipefail
shopt -s nullglob

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
DOCS_DIR="$PROJECT_ROOT/docs"

extract_title() {
    local file="$1"
    local fallback="$2"
    local title
    title=$(awk '/^# / {sub(/^# /, ""); print; exit}' "$file")
    if [[ -n "$title" ]]; then
        echo "$title"
    else
        echo "$fallback"
    fi
}

is_rfc_deprecated() {
    local rfc_id="$1"
    local status
    status=$(govctl rfc get "$rfc_id" status 2>/dev/null || true)

    [[ "$status" == "deprecated" ]]
}

is_adr_superseded() {
    local adr_id="$1"
    local status
    status=$(govctl adr get "$adr_id" status 2>/dev/null || true)

    [[ "$status" == "superseded" ]]
}

cd "$PROJECT_ROOT"

# Parse arguments
SKIP_RENDER=false
SERVE=false
for arg in "$@"; do
    case $arg in
        --skip-render) SKIP_RENDER=true ;;
        --serve) SERVE=true ;;
    esac
done

# Render governance artifacts to markdown (unless --skip-render)
if [[ "$SKIP_RENDER" == "false" ]]; then
    echo "Rendering governance artifacts..."
    govctl render all
    govctl render changelog
    # Copy changelog to docs/ for mdbook
    if [[ -f "$PROJECT_ROOT/CHANGELOG.md" ]]; then
        cp "$PROJECT_ROOT/CHANGELOG.md" "$DOCS_DIR/CHANGELOG.md"
    fi
else
    echo "Skipping render (--skip-render)"
fi

# Generate SUMMARY.md dynamically
echo "Generating SUMMARY.md..."

SUMMARY="$DOCS_DIR/SUMMARY.md"
cat > "$SUMMARY" <<'EOF_SUMMARY'
# Summary

[Introduction](./INTRODUCTION.md)

# User Guide
EOF_SUMMARY

if [[ -f "$DOCS_DIR/guide/README.md" ]]; then
    echo "- [Guide Overview](./guide/README.md)" >> "$SUMMARY"
fi

cat >> "$SUMMARY" <<'EOF_SUMMARY'
- [Getting Started](./guide/getting-started.md)
- [Adapters](./guide/adapters.md)
- [Asset Handling](./guide/assets.md)
- [Theme Customization](./guide/theme-customization.md)
- [Copy-paste Profiles](./guide/profiles.md)
- [Advanced Customization](./guide/advanced-customization.md)
- [External Storage Configuration](./guide/storage/README.md)

EOF_SUMMARY

# Add Platforms section (pure list structure for multi-level folding)
PLATFORMS_DIR="$DOCS_DIR/guide/platforms"

echo "# Platform Guides" >> "$SUMMARY"
echo "" >> "$SUMMARY"

# Level 0: Platforms Overview (foldable, has children)
if [[ -f "$PLATFORMS_DIR/README.md" ]]; then
    echo "- [Platforms Overview](./guide/platforms/README.md)" >> "$SUMMARY"
else
    echo "- Platforms Overview" >> "$SUMMARY"
fi

# Level 1: Direct Publish (API Adapters)
if [[ -f "$PLATFORMS_DIR/api-adapters.md" ]]; then
    echo "  - [Direct Publish (API Adapters)](./guide/platforms/api-adapters.md)" >> "$SUMMARY"
else
    echo "  - Direct Publish (API Adapters)" >> "$SUMMARY"
fi

# Level 2: Individual API platforms
api_platforms=(confluence devto ghost hashnode notion wordpress)
for platform in "${api_platforms[@]}"; do
    readme="$PLATFORMS_DIR/$platform/README.md"
    if [[ -f "$readme" ]]; then
        title=$(extract_title "$readme" "$platform")
        echo "    - [$title](./guide/platforms/$platform/README.md)" >> "$SUMMARY"
    fi
done

# Level 1: Local Output Adapters
if [[ -f "$PLATFORMS_DIR/local-output-adapters.md" ]]; then
    echo "  - [Local Output Adapters](./guide/platforms/local-output-adapters.md)" >> "$SUMMARY"
else
    echo "  - Local Output Adapters" >> "$SUMMARY"
fi

# Level 2: Individual local-output platforms
local_output_platforms=(astro static xiaohongshu)
for platform in "${local_output_platforms[@]}"; do
    readme="$PLATFORMS_DIR/$platform/README.md"
    if [[ -f "$readme" ]]; then
        title=$(extract_title "$readme" "$platform")
        echo "    - [$title](./guide/platforms/$platform/README.md)" >> "$SUMMARY"
    fi
done

# Level 1: Copy-paste Platforms
if [[ -f "$PLATFORMS_DIR/copy-paste-platforms.md" ]]; then
    echo "  - [Copy-paste Platforms](./guide/platforms/copy-paste-platforms.md)" >> "$SUMMARY"
else
    echo "  - Copy-paste Platforms" >> "$SUMMARY"
fi

# Level 2: HTML Copy-paste subgroup
if [[ -f "$PLATFORMS_DIR/html-copy-paste-platforms.md" ]]; then
    echo "    - [HTML Copy-paste Platforms](./guide/platforms/html-copy-paste-platforms.md)" >> "$SUMMARY"
else
    echo "    - HTML Copy-paste Platforms" >> "$SUMMARY"
fi

html_copy_paste_platforms=(wechat zhihu toutiao bilibili weibo baijiahao wangyihao sohu sspai oschina)
for platform in "${html_copy_paste_platforms[@]}"; do
    readme="$PLATFORMS_DIR/$platform/README.md"
    if [[ -f "$readme" ]]; then
        title=$(extract_title "$readme" "$platform")
        echo "      - [$title](./guide/platforms/$platform/README.md)" >> "$SUMMARY"
    fi
done

# Level 2: Markdown Copy-paste subgroup
if [[ -f "$PLATFORMS_DIR/markdown-copy-paste-platforms.md" ]]; then
    echo "    - [Markdown Copy-paste Platforms](./guide/platforms/markdown-copy-paste-platforms.md)" >> "$SUMMARY"
else
    echo "    - Markdown Copy-paste Platforms" >> "$SUMMARY"
fi

markdown_copy_paste_platforms=(51cto aliyun cnblogs csdn elecfans huaweicloud infoq jianshu juejin medium modelscope segmentfault tencentcloud volcengine)
for platform in "${markdown_copy_paste_platforms[@]}"; do
    readme="$PLATFORMS_DIR/$platform/README.md"
    if [[ -f "$readme" ]]; then
        title=$(extract_title "$readme" "$platform")
        echo "      - [$title](./guide/platforms/$platform/README.md)" >> "$SUMMARY"
    fi
done

# Add Developer Docs section
cat >> "$SUMMARY" <<'EOF_SUMMARY'

# Developer Docs

- [Developer Guide](./developer/README.md)

EOF_SUMMARY

# Add RFC section
echo "# RFC Specifications" >> "$SUMMARY"
echo "" >> "$SUMMARY"
rfc_indent=""
if [[ -f "$DOCS_DIR/rfc/README.md" ]]; then
    echo "- [RFC Index](./rfc/README.md)" >> "$SUMMARY"
    rfc_indent="  "
fi
rfc_entries=()
for rfc in "$DOCS_DIR"/rfc/RFC-*.md; do
    filename=$(basename "$rfc")
    id="${filename%.md}"
    if is_rfc_deprecated "$id"; then
        continue
    fi
    title=$(extract_title "$rfc" "$id")
    rfc_entries+=("- [$title](./$filename)")
    echo "${rfc_indent}- [$title](./rfc/$filename)" >> "$SUMMARY"
done

# Add ADR section
echo "" >> "$SUMMARY"
echo "# ADR Decisions" >> "$SUMMARY"
echo "" >> "$SUMMARY"
adr_indent=""
if [[ -f "$DOCS_DIR/adr/README.md" ]]; then
    echo "- [ADR Index](./adr/README.md)" >> "$SUMMARY"
    adr_indent="  "
fi
adr_entries=()
for adr in "$DOCS_DIR"/adr/ADR-*.md; do
    filename=$(basename "$adr")
    id="${filename%.md}"
    if is_adr_superseded "$id"; then
        continue
    fi
    title=$(extract_title "$adr" "$id")
    adr_entries+=("- [$title](./$filename)")
    echo "${adr_indent}- [$title](./adr/$filename)" >> "$SUMMARY"
done

# Generate RFC/ADR index pages with deprecated entries filtered out.
cat > "$DOCS_DIR/rfc/README.md" <<'EOF_RFC_INDEX'
# RFC Index

This section contains normative specifications for typub behavior.

## Suggested Reading Order

EOF_RFC_INDEX

if [[ ${#rfc_entries[@]} -gt 0 ]]; then
    printf '%s\n' "${rfc_entries[@]}" >> "$DOCS_DIR/rfc/README.md"
else
    echo "- No active RFCs found." >> "$DOCS_DIR/rfc/README.md"
fi

cat > "$DOCS_DIR/adr/README.md" <<'EOF_ADR_INDEX'
# ADR Index

This section records major architecture and implementation decisions.

## Suggested Reading Order

EOF_ADR_INDEX

if [[ ${#adr_entries[@]} -gt 0 ]]; then
    printf '%s\n' "${adr_entries[@]}" >> "$DOCS_DIR/adr/README.md"
else
    echo "- No active ADRs found." >> "$DOCS_DIR/adr/README.md"
fi

# Add Changelog at the end
if [[ -f "$DOCS_DIR/CHANGELOG.md" ]]; then
    cat >> "$SUMMARY" <<'EOF_SUMMARY'

---

[Changelog](./CHANGELOG.md)
EOF_SUMMARY
fi

echo "Generated: $SUMMARY"

# Build or serve
cd "$DOCS_DIR"
if [[ "$SERVE" == "true" ]]; then
    echo "Starting mdbook server..."
    mdbook serve --open
else
    echo "Building mdbook..."
    mdbook build
    echo "Book built: $DOCS_DIR/book/"
fi
