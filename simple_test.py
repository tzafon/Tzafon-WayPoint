import asyncio
from tzafonwright import TzafonWrightClient, Command, ActionType

async def main():
    # Connect to your running Tzafon-WayPoint proxy (replace with actual URL)
    proxy_url = "ws://localhost:1337"

    async with TzafonWrightClient(ws_url=proxy_url) as tw:
        
        # Navigate to example.com
        navigation_command = Command(action_type=ActionType.GOTO, url="https://example.com")
        navigation_result = await tw.send_action(navigation_command)

        print("Navigated to example.com. Result: ", navigation_result)

        # Take screenshot
        screenshot_command = Command(action_type=ActionType.SCREENSHOT)
        screenshot_result = await tw.send_action(screenshot_command)

        with open("example.jpg", "wb") as f:
            f.write(screenshot_result.image)

        print("Screenshot saved as example.jpg")

if __name__ == "__main__":
    asyncio.run(main())