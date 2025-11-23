---
description: Debug Tailwind CSS styling issues using automated browser screenshots
---

Help debug Tailwind CSS styling issues using the browser screenshot tool.

When the user mentions styling issues, Tailwind problems, or layout issues, follow this workflow:

1. **Understand the issue:**
   - Ask which component or element is having the issue
   - Ask what the expected vs actual behavior is
   - Ask which app (admin/frontend) if not clear

2. **Set up screenshot comparison:**
   - Ask which app to debug (admin app on 5173 or frontend)
   - Check if that dev server is running
   - If not, tell user how to start it
   - Launch screenshot tool with correct URL: `poetry run python screenshot_browser.py --no-headless --url [URL]`

3. **Capture baseline:**
   - Take a screenshot of the current state
   - Use specific selectors when possible (e.g., `element .header header-before`)

4. **Investigate the code:**
   - Read the relevant component file
   - Look for CSS classes, Tailwind classes, or inline styles
   - Check for potential conflicts or issues

5. **Suggest fixes:**
   - Propose specific CSS/styling changes
   - Explain why the issue might be happening
   - Offer multiple solutions when applicable

6. **Verify the fix:**
   - After user makes changes (or you edit the file)
   - Instruct to reload in screenshot tool
   - Capture new screenshot for comparison
   - Compare before/after screenshots

7. **Iterate if needed:**
   - If the fix doesn't work, try alternative approaches
   - Use browser dev tools insights
   - Consider different CSS patterns or layout approaches

Common Tailwind CSS debugging patterns:
- **Classes not applying:**
  - Check for typos in class names
  - Verify Tailwind is compiling (check terminal for errors)
  - Check if using Tailwind v4 syntax correctly
  - Look for conflicting classes on the same element

- **Padding/margin not visible:**
  - Content might be stretching with flex
  - Use max-width containers for better control
  - Try increasing the value (px-4 → px-8 → px-12) to see if it's just too small

- **Layout issues:**
  - Check flex/grid classes (flex, items-center, justify-between, etc.)
  - Verify parent constraints (h-screen, w-full, etc.)
  - Look for conflicting positioning (absolute, fixed, relative)

- **Responsive classes not working:**
  - Verify breakpoint syntax (sm:, md:, lg:, xl:)
  - Check viewport size matches the breakpoint
  - Ensure mobile-first approach (base styles, then override)

- **Colors/opacity not applying:**
  - Check Tailwind v4 theme configuration in index.css
  - Verify color format (bg-zinc-800, text-white, etc.)
  - Check opacity syntax (bg-black/60, border-zinc-800/60)

Remember: The screenshot tool makes this process fast - you can iterate quickly and visually compare changes!
