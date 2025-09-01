#!/bin/bash
# ABOUTME: Wrapper script to handle Claude CLI initial prompts automatically
# This script answers "1" to both trust dialog and theme selection if they appear

# Use expect if available, otherwise fallback
if command -v expect >/dev/null 2>&1; then
    # Use expect to handle both trust dialog and theme selection
    expect -c "
        set timeout 5
        spawn /home/claude-user/.npm-global/bin/claude
        expect {
            \"Yes, proceed\" {
                send \"1\r\"
                exp_continue
            }
            \"Choose the text style\" {
                sleep 1
                send \"1\r\"
                interact
            }
            \"Dark mode\" {
                send \"1\r\"
                interact
            }
            timeout {
                interact
            }
        }
    "
else
    # Fallback: just run Claude directly
    # The dialogs will appear but at least Claude will run
    exec /home/claude-user/.npm-global/bin/claude
fi