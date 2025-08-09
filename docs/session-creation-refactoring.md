# Session Creation Architecture Refactoring

## Problem Statement

The current session creation system has **two separate code paths** that implement similar functionality, leading to code duplication, inconsistent behavior, and maintenance issues.

## Current Architecture Issues

### Dual Path Problem

#### Path 1: Claude-Dev Specific (Legacy)

**Location**: `src/docker/claude_dev.rs` + specific methods in `src/docker/session_lifecycle.rs`

```rust
// session_lifecycle.rs:84-150
pub async fn create_claude_dev_session_with_logs() {
    // Calls claude_dev.rs directly
    let container_id = super::create_claude_dev_session(&worktree_info.path, claude_dev_config, request.session_id, progress_sender, mount_claude_config).await?;
}
```

**Characteristics**:

- Hardcoded for "claude-dev" containers only
- Uses `ClaudeDevManager` class
- Custom mounting logic in `claude_dev.rs:300-306`
- Bypasses the template system entirely
- Direct container creation without project config integration

#### Path 2: Generic Template System (Current)

**Location**: `src/docker/session_lifecycle.rs:227-320`

```rust
// Load project config and determine template
let template_name = project_config
    .as_ref()
    .and_then(|pc| pc.container_template.as_ref())
    .map(|s| s.as_str())
    .unwrap_or(&self.app_config.default_container_template);

if let Some(template) = self.app_config.get_container_template(template_name) {
    let mut config = template.to_container_config();
    // Apply project overrides, MCP, mounting, etc.
}
```

**Characteristics**:

- Works with any container template
- Respects project configuration (`project.toml`)
- Template-driven approach
- Integrated MCP server initialization
- Proper mounting logic with configuration support

### Concrete Problems Encountered

1. **Bug We Hit**: `.claude.json` mounting logic was only implemented in Path 1, but sessions were using Path 2
2. **Code Duplication**: Mounting logic implemented twice with different behavior
3. **Inconsistent Features**: Path 1 has features that Path 2 lacks and vice versa
4. **Maintenance Burden**: Bug fixes need to be applied in multiple places
5. **Testing Complexity**: Need to test both paths independently
6. **Developer Confusion**: Unclear which path will be used for a given scenario

### Path Selection Logic (Current)

The path selection appears to be:

- **Path 1**: Direct calls to `create_claude_dev_session()` (unclear when this happens)
- **Path 2**: When project config exists with `container_template` specified (most common case)

This selection logic is **implicit and undocumented**, making it impossible to predict behavior.

## Proposed Solution: Unified Session Creation

### Target Architecture

```rust
// Single entry point
pub async fn create_session(
    &mut self,
    request: SessionRequest,
    progress_sender: Option<mpsc::Sender<SessionProgress>>
) -> Result<SessionState, SessionLifecycleError> {
    // 1. Load and validate configuration
    let (project_config, template) = self.load_session_configuration(&request)?;

    // 2. Create base container configuration from template
    let mut container_config = template.to_container_config();

    // 3. Apply project-specific overrides
    self.apply_project_overrides(&mut container_config, &project_config);

    // 4. Initialize MCP servers
    let mcp_result = self.initialize_mcp_servers(&mut container_config, &request, &project_config).await?;

    // 5. Apply mounting logic (unified for all templates)
    self.apply_mounting_logic(&mut container_config, &project_config, &mcp_result)?;

    // 6. Create and start container
    let container = self.create_and_start_container(request.session_id, container_config, progress_sender).await?;

    // 7. Return session state
    Ok(self.create_session_state(request, container))
}
```

### Key Principles

1. **Single Path**: One method handles all session creation
2. **Template-Driven**: All container types defined as templates
3. **Configuration-Aware**: Respects project and global configuration
4. **Extensible**: Easy to add new container types
5. **Testable**: Single path to test and maintain
6. **Predictable**: Clear, documented behavior

### Implementation Plan

#### Phase 1: Preparation

- [ ] Create unified session progress enum
- [ ] Extract common functionality into helper methods
- [ ] Create comprehensive test suite for current behavior
- [ ] Document current path selection logic

#### Phase 2: Template System Enhancement

- [ ] Ensure claude-dev template has all required features
- [ ] Migrate claude-dev specific logic to template configuration
- [ ] Add template validation
- [ ] Create template-specific configuration options

#### Phase 3: Unified Implementation

- [ ] Implement `create_session()` method
- [ ] Migrate mounting logic to unified approach
- [ ] Integrate MCP initialization
- [ ] Add comprehensive error handling

#### Phase 4: Migration and Cleanup

- [ ] Update all callers to use new unified method
- [ ] Mark old methods as deprecated
- [ ] Remove claude-dev specific path
- [ ] Update tests to use unified approach

#### Phase 5: Validation

- [ ] End-to-end testing with all container types
- [ ] Performance validation
- [ ] Documentation updates
- [ ] User acceptance testing

### Breaking Changes

#### For End Users

- **None expected**: The unified approach should maintain all current functionality
- Project configurations should continue to work unchanged
- All container templates should behave identically

#### For Developers

- **API Changes**: `create_claude_dev_session()` methods will be deprecated
- **Import Changes**: Some internal modules may be restructured
- **Test Changes**: Test code will need to use new unified methods

### Benefits

#### Immediate Benefits

- **Bug Prevention**: Mounting logic bugs like we encountered won't happen
- **Consistency**: All templates behave identically
- **Maintainability**: Single place to implement features

#### Long-term Benefits

- **Extensibility**: Easy to add new container types (golang, rust, etc.)
- **Performance**: Reduced code paths and complexity
- **Documentation**: Clear, single behavior to document
- **Testing**: Simpler test matrix

### Risks and Mitigation

#### Risk: Breaking Existing Functionality

**Mitigation**:

- Comprehensive test suite before refactoring
- Gradual migration with backward compatibility
- Thorough end-to-end testing

#### Risk: Performance Regression

**Mitigation**:

- Benchmark current performance
- Optimize unified path
- Monitor performance during migration

#### Risk: Extended Development Time

**Mitigation**:

- Phased approach allows incremental progress
- Keep old path working during migration
- Focus on high-impact areas first

## Current Status

### Immediate Fix Applied

- Added `.claude.json` mounting logic to both paths for consistency
- This provides a working solution while we plan the refactoring

### Next Steps

1. **Review and approve this plan**
2. **Create GitHub issues for each phase**
3. **Begin Phase 1 implementation**
4. **Set up tracking for the refactoring effort**

## Files Affected

### Core Session Management

- `src/docker/session_lifecycle.rs` - Main refactoring target
- `src/docker/claude_dev.rs` - Will be simplified/removed
- `src/docker/mod.rs` - API updates

### Configuration System

- `src/config/mod.rs` - Template system enhancements
- `src/config/container.rs` - Template validation

### Testing

- `src/docker/session_lifecycle_tests.rs` - New comprehensive tests
- `src/docker/claude_dev_tests.rs` - Migration to unified tests

### Documentation

- `CLAUDE.md` - Usage documentation updates
- `README.md` - Architecture documentation updates

---

## Conclusion

This refactoring addresses a fundamental architectural issue that has caused bugs, confusion, and maintenance overhead. The unified approach will provide a more robust, maintainable, and extensible foundation for the session creation system.

The phased approach minimizes risk while providing clear milestones and the ability to validate progress incrementally.
