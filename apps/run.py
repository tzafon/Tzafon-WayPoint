import asyncio
from tzafonwright import TzafonWrightClient, Command, ActionType

async def main():
    # Connect to your running Tzafon-WayPoint proxy (replace with actual URL)
    proxy_url = "ws://localhost:1337" 
    try:
        async with TzafonWrightClient(ws_url=proxy_url) as tw:
            print("Connecting...")
            # Navigate to example.com
            navigation_command = Command(action_type=ActionType.GOTO, url="https://example.com")
            navigation_result = await tw.send_action(navigation_command)
            if not navigation_result.success:
                raise Exception(f"Navigation failed: {navigation_result.error_message}")
            print("Navigated to example.com")

            # Take screenshot
            screenshot_command = Command(action_type=ActionType.SCREENSHOT)
            screenshot_result = await tw.send_action(screenshot_command)
            if screenshot_result.success and screenshot_result.image:
                with open("example.jpg", "wb") as f:
                    f.write(screenshot_result.image)
                print("Screenshot saved as example.jpg")
            else:
                raise Exception(f"Screenshot failed: {screenshot_result.error_message or 'No image data'}")

    except Exception as e:
        print(f"An error occurred: {e}")

if __name__ == "__main__":
    asyncio.run(main())