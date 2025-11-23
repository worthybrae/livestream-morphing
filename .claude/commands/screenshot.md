---
description: Launch interactive browser screenshot tool to debug UI and styling issues
---

Launch the browser screenshot tool to help debug UI and styling issues visually.

Follow these steps:

1. Ask the user which app they want to screenshot:
   - Admin app (default: http://localhost:5173)
   - Frontend app (ask for port/URL)
   - Other URL
2. Check if that server is running (if local)
3. If not running, inform the user how to start it
4. Launch the screenshot tool: `poetry run python screenshot_browser.py --no-headless --url [URL]`
4. Wait for the tool to start and show the command prompt
5. Inform the user that the browser is ready and they can now:
   - Use `screenshot [name]` to capture viewport
   - Use `element [selector] [name]` to capture specific elements
   - Use `fullpage [name]` to capture full scrollable page
   - Use `reload` to refresh after making code changes
   - Use `quit` to exit

Example workflow:
- `element .h-12 header-before` - capture header before changes
- Make changes to code (Vite will hot reload)
- `reload` - ensure changes are loaded
- `element .h-12 header-after` - capture header after changes
- Compare screenshots in the screenshots/ folder

The tool is perfect for debugging CSS issues, comparing before/after changes, and visual regression testing.
