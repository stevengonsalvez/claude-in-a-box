# Boss Mode Keyboard Shortcuts

This document describes the enhanced keyboard shortcuts available in Boss Mode prompt input, providing a rich text editing experience with VIM-style navigation.

## Text Input Controls

### Multiline Text Support
- **Ctrl+J**: Insert new line (allows for multiline prompts)
- **Regular typing**: Insert characters at cursor position
- **Backspace**: Delete character before cursor, or join lines if at beginning of line

### Cursor Movement (VIM-style)
- **h** or **Left Arrow**: Move cursor left (previous character)
- **l** or **Right Arrow**: Move cursor right (next character)  
- **k** or **Up Arrow**: Move cursor up (previous line)
- **j** or **Down Arrow**: Move cursor down (next line)

## File Reference System
- **@**: Activate fuzzy file finder for quick file references
- **File Finder Navigation** (when @ is typed):
  - **Up/Down arrows** or **j/k**: Navigate through file list
  - **Enter**: Select highlighted file and insert path
  - **Esc**: Cancel file finder and return to normal editing
  - **Type characters**: Filter files by name

## Session Controls
- **Enter**: Proceed to permissions configuration (if prompt is not empty)
- **Esc**: Cancel session creation and return to main view

## Example Usage

### Multiline Prompt
```
Fix the authentication bug in the login system

Details:
- Users can't log in with valid credentials
- Check session handling
- Review token validation

@src/auth/login.js
```

### Quick File Reference
```
Review the implementation in @src/components/Header.tsx and suggest improvements
```

## Integration with Claude Code

These keyboard shortcuts are specifically designed for Boss Mode, which allows you to:

1. **Define complex tasks** with multiline prompts
2. **Reference specific files** using the @ file finder
3. **Navigate and edit** your prompt with precision
4. **Execute immediately** when the container starts

The enhanced text editing capabilities make Boss Mode ideal for:
- Complex code review requests
- Multi-step development tasks
- Documentation generation
- Debugging investigations
- Architecture analysis

For general Claude Code keyboard shortcuts, see the [Interactive Mode documentation](https://docs.anthropic.com/en/docs/claude-code/interactive-mode).