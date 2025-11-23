#!/usr/bin/env python3
"""
Automated browser screenshot tool for visual testing and debugging.

This script launches a browser, navigates to the admin app, and takes screenshots.
Useful for debugging UI issues, creating documentation, or visual regression testing.
"""

import asyncio
import sys
from datetime import datetime
from pathlib import Path
from typing import Optional

try:
    from playwright.async_api import async_playwright, Browser, Page
except ImportError:
    print("Error: playwright is not installed.")
    print("Install it with: poetry add playwright --group dev")
    print("Then run: poetry run playwright install")
    sys.exit(1)


class BrowserScreenshotter:
    """Automated browser screenshot tool."""

    def __init__(
        self,
        url: str = "http://localhost:5173",
        output_dir: str = "screenshots",
        headless: bool = False,
    ):
        self.url = url
        self.output_dir = Path(output_dir)
        self.headless = headless
        self.output_dir.mkdir(exist_ok=True)

    async def take_screenshot(
        self,
        page: Page,
        name: str,
        full_page: bool = False,
        selector: Optional[str] = None,
    ) -> Path:
        """
        Take a screenshot of the page or a specific element.

        Args:
            page: The Playwright page object
            name: Name for the screenshot file (without extension)
            full_page: Whether to capture the full scrollable page
            selector: CSS selector for a specific element to screenshot

        Returns:
            Path to the saved screenshot
        """
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        filename = f"{name}_{timestamp}.png"
        filepath = self.output_dir / filename

        if selector:
            # Screenshot a specific element
            element = await page.wait_for_selector(selector, timeout=5000)
            await element.screenshot(path=str(filepath))
            print(f"📸 Captured element '{selector}': {filepath}")
        else:
            # Screenshot the full page or viewport
            await page.screenshot(path=str(filepath), full_page=full_page)
            print(f"📸 Captured {'full page' if full_page else 'viewport'}: {filepath}")

        return filepath

    async def run_interactive(self):
        """
        Launch browser in interactive mode for manual testing and screenshots.
        """
        async with async_playwright() as p:
            print(f"🚀 Launching browser at {self.url}...")
            browser = await p.chromium.launch(headless=self.headless)
            context = await browser.new_context(
                viewport={"width": 1920, "height": 1080},
                device_scale_factor=2,  # Retina/HiDPI
            )
            page = await context.new_page()

            try:
                print(f"🌐 Navigating to {self.url}...")
                await page.goto(self.url, wait_until="networkidle", timeout=10000)
                await asyncio.sleep(1)  # Let any animations settle

                print("\n✅ Browser ready!")
                print("\nCommands:")
                print("  screenshot [name]      - Take a viewport screenshot")
                print("  fullpage [name]        - Take a full-page screenshot")
                print("  element [selector] [name] - Screenshot a specific element")
                print("  goto [url]            - Navigate to a different URL")
                print("  reload                - Reload the page")
                print("  resize [width] [height] - Resize the viewport")
                print("  wait [seconds]        - Wait for N seconds")
                print("  quit                  - Close browser and exit")
                print()

                while True:
                    try:
                        command = input("🎬 > ").strip()
                        if not command:
                            continue

                        parts = command.split(maxsplit=2)
                        cmd = parts[0].lower()

                        if cmd == "quit":
                            break

                        elif cmd == "screenshot":
                            name = parts[1] if len(parts) > 1 else "screenshot"
                            await self.take_screenshot(page, name)

                        elif cmd == "fullpage":
                            name = parts[1] if len(parts) > 1 else "fullpage"
                            await self.take_screenshot(page, name, full_page=True)

                        elif cmd == "element":
                            if len(parts) < 2:
                                print("❌ Usage: element [selector] [name]")
                                continue
                            selector = parts[1]
                            name = parts[2] if len(parts) > 2 else "element"
                            await self.take_screenshot(page, name, selector=selector)

                        elif cmd == "goto":
                            if len(parts) < 2:
                                print("❌ Usage: goto [url]")
                                continue
                            url = parts[1]
                            await page.goto(url, wait_until="networkidle")
                            print(f"✅ Navigated to {url}")

                        elif cmd == "reload":
                            await page.reload(wait_until="networkidle")
                            print("✅ Page reloaded")

                        elif cmd == "resize":
                            if len(parts) < 3:
                                print("❌ Usage: resize [width] [height]")
                                continue
                            width = int(parts[1])
                            height = int(parts[2])
                            await page.set_viewport_size({"width": width, "height": height})
                            print(f"✅ Viewport resized to {width}x{height}")

                        elif cmd == "wait":
                            if len(parts) < 2:
                                print("❌ Usage: wait [seconds]")
                                continue
                            seconds = float(parts[1])
                            await asyncio.sleep(seconds)
                            print(f"✅ Waited {seconds}s")

                        else:
                            print(f"❌ Unknown command: {cmd}")

                    except KeyboardInterrupt:
                        print("\n\nUse 'quit' to exit")
                    except Exception as e:
                        print(f"❌ Error: {e}")

            finally:
                await browser.close()
                print("👋 Browser closed")

    async def run_automated(
        self,
        screenshots: list[dict],
        wait_time: float = 2.0,
    ):
        """
        Run automated screenshot capture.

        Args:
            screenshots: List of screenshot configs, each with:
                - name: Screenshot name
                - selector: (optional) CSS selector to screenshot
                - full_page: (optional) Whether to capture full page
                - wait: (optional) Seconds to wait before screenshot
            wait_time: Default wait time between actions
        """
        async with async_playwright() as p:
            print(f"🚀 Launching browser at {self.url}...")
            browser = await p.chromium.launch(headless=self.headless)
            context = await browser.new_context(
                viewport={"width": 1920, "height": 1080},
                device_scale_factor=2,
            )
            page = await context.new_page()

            try:
                print(f"🌐 Navigating to {self.url}...")
                await page.goto(self.url, wait_until="networkidle", timeout=10000)
                await asyncio.sleep(wait_time)

                for config in screenshots:
                    name = config["name"]
                    selector = config.get("selector")
                    full_page = config.get("full_page", False)
                    wait = config.get("wait", wait_time)

                    if wait > 0:
                        await asyncio.sleep(wait)

                    await self.take_screenshot(page, name, full_page, selector)

                print(f"\n✅ Captured {len(screenshots)} screenshots")

            finally:
                await browser.close()
                print("👋 Browser closed")


async def main():
    """Main entry point."""
    import argparse

    parser = argparse.ArgumentParser(
        description="Automated browser screenshot tool",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Interactive mode (recommended for debugging)
  python screenshot_browser.py

  # Automated mode with predefined screenshots
  python screenshot_browser.py --auto

  # Custom URL and visible browser
  python screenshot_browser.py --url http://localhost:3000 --no-headless

  # Custom output directory
  python screenshot_browser.py --output my-screenshots
        """,
    )
    parser.add_argument(
        "--url",
        default="http://localhost:5173",
        help="URL to navigate to (default: http://localhost:5173)",
    )
    parser.add_argument(
        "--output",
        default="screenshots",
        help="Output directory for screenshots (default: screenshots)",
    )
    parser.add_argument(
        "--headless",
        action="store_true",
        default=False,
        help="Run browser in headless mode",
    )
    parser.add_argument(
        "--no-headless",
        action="store_true",
        help="Run browser with visible UI (opposite of --headless)",
    )
    parser.add_argument(
        "--auto",
        action="store_true",
        help="Run in automated mode with predefined screenshots",
    )

    args = parser.parse_args()

    # Handle headless flag
    headless = args.headless and not args.no_headless

    screenshotter = BrowserScreenshotter(
        url=args.url,
        output_dir=args.output,
        headless=headless,
    )

    if args.auto:
        # Automated mode - take predefined screenshots
        screenshots = [
            {"name": "full-app", "full_page": False, "wait": 1},
            {"name": "header", "selector": "header, .h-12", "wait": 0.5},
            {"name": "video-preview", "selector": ".video-preview, video", "wait": 0.5},
        ]
        await screenshotter.run_automated(screenshots)
    else:
        # Interactive mode
        await screenshotter.run_interactive()


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\n\n👋 Goodbye!")
        sys.exit(0)
