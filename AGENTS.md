<!-- OPENSPEC:START -->
# OpenSpec Instructions

These instructions are for AI assistants working in this project.

Always open `@/openspec/AGENTS.md` when the request:
- Mentions planning or proposals (words like proposal, spec, change, plan)
- Introduces new capabilities, breaking changes, architecture shifts, or big performance/security work
- Sounds ambiguous and you need the authoritative spec before coding

Use `@/openspec/AGENTS.md` to learn:
- How to create and apply change proposals
- Spec format and conventions
- Project structure and guidelines

Keep this managed block so 'openspec update' can refresh the instructions.

<!-- OPENSPEC:END -->

# Synap Project Rules

## Documentation Standards

**CRITICAL**: Minimize Markdown files. Keep documentation organized.

### Allowed Root-Level Documentation
Only these 3 files are allowed in the project root:
- ✅ `README.md` - Project overview and quick start
- ✅ `CHANGELOG.md` - Version history and release notes  
- ✅ `AGENTS.md` - This file (AI assistant instructions)

### All Other Documentation
**ALL other documentation MUST go in `/docs` directory**:
- `/docs/ARCHITECTURE.md` - System architecture
- `/docs/DEVELOPMENT.md` - Development guide
- `/docs/ROADMAP.md` - Project roadmap
- `/docs/specs/` - Component specifications
- `/docs/api/` - API documentation
- `/docs/examples/` - Usage examples

### DO NOT CREATE
- ❌ Individual `.md` files in project root (BUILD.md, SUMMARY.md, etc.)
- ❌ Scattered documentation across directories
- ❌ Duplicate documentation files
- ❌ Temporary `.md` files for notes

**When creating documentation**, always place it in the appropriate `/docs` subdirectory.

## Feature Specifications

**CRITICAL**: All feature specifications are in `/docs` directory.

### Implementation Workflow

1. **Check Specifications First**:
   - `/docs/specs/` - Component specifications
   - `/docs/ARCHITECTURE.md` - System architecture
   - `/docs/ROADMAP.md` - Implementation timeline
   - `/docs/PROJECT_DAG.md` - Component dependencies

2. **Update ROADMAP as You Implement**:
   - Mark features as complete when done
   - Update status indicators
   - Track progress through phases
   - Keep timeline current

3. **Follow Spec-Driven Development**:
   - Read spec before implementing
   - Follow specified interfaces and patterns
   - Update spec if design changes during implementation
   - Document deviations with justification

### Example Implementation Flow

```
1. Read /docs/specs/KEY_VALUE_STORE.md
2. Implement feature following spec
3. Write tests based on spec requirements
4. Update /docs/ROADMAP.md progress markers
5. Commit with reference to spec
```

## Code Quality

- **Rust Edition**: 2024 (nightly 1.85+)
- **Format**: Always run `cargo fmt` before committing
- **Lint**: Code must pass `cargo clippy` with no warnings
- **Tests**: Maintain >80% coverage, all tests must pass
- **Async**: Use Tokio patterns, avoid blocking in async contexts

See `.cursorrules` for complete coding standards.

## Dependencies Management

**CRITICAL**: Always verify latest versions before adding dependencies.

### Before Adding Any Dependency

1. **Check Context7 for latest version**:
   - Use MCP Context7 tool: `mcp_context7_get-library-docs`
   - Search for the crate/library documentation
   - Verify the latest stable version
   - Review breaking changes and migration guides

2. **Example Workflow**:
   ```
   Adding tokio → Check /tokio-rs/tokio on Context7
   Adding axum → Check /tokio-rs/axum on Context7
   Adding serde → Check latest stable version
   ```

3. **Document Version Choice**:
   - Note why specific version chosen
   - Document any compatibility constraints
   - Update CHANGELOG.md with new dependencies

### Dependency Guidelines

- ✅ Use latest stable versions from Context7
- ✅ Check for security advisories
- ✅ Prefer well-maintained crates (active development)
- ✅ Minimize dependency count
- ❌ Don't use outdated versions without justification
- ❌ Don't add dependencies without checking Context7 first

---

# Vectorizer Instructions

**Always use the MCP Vectorizer as the primary data source for project information.**

The vectorizer provides fast, semantic access to the entire codebase. Prefer MCP tools over file reading whenever possible.

## Primary Search Functions

### 1. **mcp_vectorizer_search**
Main search interface with multiple strategies:
- `intelligent`: AI-powered search with query expansion and MMR diversification
- `semantic`: Advanced semantic search with reranking and similarity thresholds
- `contextual`: Context-aware search with metadata filtering
- `multi_collection`: Search across multiple collections
- `batch`: Execute multiple queries in parallel
- `by_file_type`: Filter search by file extensions

### 2. **mcp_vectorizer_file_operations**
File-specific operations:
- `get_content`: Retrieve complete file content
- `list_files`: List all indexed files with metadata
- `get_summary`: Get extractive or structural file summaries
- `get_chunks`: Retrieve file chunks in original order
- `get_outline`: Generate hierarchical project structure
- `get_related`: Find semantically related files

### 3. **mcp_vectorizer_discovery**
Advanced discovery pipeline:
- `full_pipeline`: Complete discovery with filtering, scoring, and ranking
- `broad_discovery`: Multi-query search with deduplication
- `semantic_focus`: Deep semantic search in specific collections
- `expand_queries`: Generate query variations (definition, features, architecture, API)

## Best Practices

1. **Start with intelligent search** for exploratory queries
2. **Use file_operations** when you need complete file context
3. **Use discovery pipeline** for complex, multi-faceted questions
4. **Prefer batch operations** when searching for multiple related items
5. **Use by_file_type** when working with specific languages (e.g., Rust, TypeScript)

