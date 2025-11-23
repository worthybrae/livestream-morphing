# Browser Screenshot Tool

Automated browser screenshot tool for visual testing, debugging UI issues, and creating documentation.

## Quick Start

1. **Install dependencies:**
   ```bash
   poetry install
   poetry run playwright install
   ```

2. **Start your admin app:**
   ```bash
   make admin-dev
   # or: cd admin-app && npm run dev
   ```

3. **Run the screenshot tool:**
   ```bash
   # Interactive mode (recommended for debugging)
   make screenshot

   # Automated mode
   make screenshot-auto
   ```

## Interactive Mode

The interactive mode opens a browser window and gives you a command prompt to control screenshots:

```bash
poetry run python screenshot_browser.py --no-headless
```

### Available Commands

- `screenshot [name]` - Capture the current viewport
- `fullpage [name]` - Capture the entire scrollable page
- `element [selector] [name]` - Capture a specific element
- `goto [url]` - Navigate to a different URL
- `reload` - Reload the current page
- `resize [width] [height]` - Change viewport size
- `wait [seconds]` - Wait before next action
- `quit` - Exit the tool

### Example Session

```bash
🎬 > screenshot homepage
📸 Captured viewport: screenshots/homepage_20250122_143022.png

🎬 > element .header header-component
📸 Captured element '.header': screenshots/header-component_20250122_143045.png

🎬 > resize 375 667
✅ Viewport resized to 375x667

🎬 > screenshot mobile-view
📸 Captured viewport: screenshots/mobile-view_20250122_143102.png

🎬 > quit
```

## Automated Mode

Run predefined screenshot sequences automatically:

```bash
poetry run python screenshot_browser.py --auto --headless
```

Edit `screenshot_browser.py` to customize the automated screenshots:

```python
screenshots = [
    {"name": "full-app", "full_page": False, "wait": 1},
    {"name": "header", "selector": ".header", "wait": 0.5},
    {"name": "video-section", "selector": ".video-preview", "wait": 0.5},
]
```

## Command Line Options

```bash
python screenshot_browser.py [OPTIONS]

Options:
  --url URL           URL to navigate to (default: http://localhost:5173)
  --output DIR        Output directory (default: screenshots)
  --headless          Run browser in headless mode
  --no-headless       Show browser window (useful for debugging)
  --auto              Run automated screenshot sequence
```

## Common Use Cases

### Debug CSS Issues

```bash
# Open browser, inspect the page, take screenshots
poetry run python screenshot_browser.py --no-headless

# In the tool:
🎬 > element .h-12 header-with-padding
🎬 > resize 1920 1080
🎬 > screenshot full-width
```

### Visual Regression Testing

```bash
# Capture baseline screenshots
poetry run python screenshot_browser.py --auto

# Make CSS changes, then capture new screenshots
poetry run python screenshot_browser.py --auto

# Compare the images manually or with a diff tool
```

### Mobile Responsive Testing

```bash
poetry run python screenshot_browser.py --no-headless

🎬 > resize 375 667
🎬 > screenshot mobile-iphone
🎬 > resize 768 1024
🎬 > screenshot tablet-ipad
🎬 > resize 1920 1080
🎬 > screenshot desktop
```

### Documentation Screenshots

```bash
poetry run python screenshot_browser.py --no-headless

🎬 > wait 2
🎬 > fullpage app-overview
🎬 > element .code-editor editor-component
🎬 > element .video-preview preview-component
```

## Tips

1. **High-DPI Screenshots**: The tool captures at 2x device scale factor by default for crisp screenshots

2. **Wait for Animations**: Use `wait [seconds]` to let animations complete before capturing

3. **Selector Tips**:
   - Use CSS selectors: `.class`, `#id`, `header`, `[data-testid="..."]`
   - For multiple matches, the first element is captured
   - Use browser dev tools to find the right selector

4. **Debugging CSS**:
   - Use `--no-headless` to see the browser
   - Take screenshots before and after CSS changes
   - Capture specific elements to isolate issues

5. **Automation**:
   - Start with interactive mode to find the right selectors
   - Then automate with `--auto` mode
   - Integrate into CI/CD for visual regression testing

## Troubleshooting

**Browser doesn't start:**
```bash
poetry run playwright install
```

**Can't connect to admin app:**
- Make sure `make admin-dev` is running
- Check that it's on http://localhost:5173
- Use `--url` to specify a different URL

**Element not found:**
- Use browser dev tools to verify the selector
- Add a `wait` to let the page load
- Use more specific selectors

## Integration with Development Workflow

Add to your workflow:

1. **Before making CSS changes**: Capture baseline screenshots
2. **While debugging**: Use interactive mode to inspect elements
3. **After changes**: Capture new screenshots and compare
4. **Before commits**: Run automated screenshots for visual regression

Example workflow:
```bash
# Terminal 1: Start admin app
make admin-dev

# Terminal 2: Debug styling
make screenshot

# In screenshot tool:
🎬 > element .h-12 header-before
# Make CSS changes in your editor
🎬 > reload
🎬 > element .h-12 header-after
# Compare screenshots/header-before_*.png vs header-after_*.png
```
