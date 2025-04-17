import asyncio
import websockets
import argparse
import traceback
from typing import Optional
import signal
from playwright.async_api import (
    async_playwright,
    Error as PlaywrightError,
    Page,
    BrowserContext,
    Browser,
    Playwright,
)
from tzafonwright.models import Command, Result, ActionType


# --- Browser Server (Playwright) ---


class BrowserServer:
    """WebSocket server for TzafonWright using Playwright for browser automation."""

    def __init__(self, port: int, cdp_url: str):
        self.port = port
        self.cdp_url = cdp_url

        self._playwright_instance: Optional[Playwright] = None
        self._browser: Optional[Browser] = None
        self._context: Optional[BrowserContext] = None
        self._page: Optional[Page] = None
        self._server_task: Optional[asyncio.Task] = None
        self._stop_event = asyncio.Event()

    async def _initialize_playwright(self):
        """Initializes the Playwright backend."""
        print("[BrowserServer] Initializing Playwright backend...")
        if not async_playwright:
            raise RuntimeError("Playwright is selected but not available.")

        try:
            print("[BrowserServer] Starting Playwright instance...")
            self._playwright_instance = await async_playwright().start()

            print(
                f"[BrowserServer] Attempting Playwright connect_over_cdp to {self.cdp_url}"
            )
            self._browser = await self._playwright_instance.chromium.connect_over_cdp(
                self.cdp_url
            )
            print("[BrowserServer] Connected via CDP.")

            if not self._browser.contexts:
                print(
                    "[BrowserServer] No contexts found in existing browser via CDP, creating one."
                )
                self._context = await self._browser.new_context()
            else:
                print("[BrowserServer] Using existing context from browser via CDP.")
                self._context = self._browser.contexts[0]

            if not self._context.pages:
                print("[BrowserServer] No pages found in context, creating one.")
                self._page = await self._context.new_page()
            else:
                print("[BrowserServer] Using existing page from context.")
                self._page = self._context.pages[0]

            print(
                f"[BrowserServer] Playwright Page ready: {self._page.url if self._page else 'No Page Yet'}"
            )

        except PlaywrightError as e:
            print(f"[BrowserServer] Playwright Error during initialization: {e}")
            traceback.print_exc()
            await self.stop()
            raise
        except Exception as e:
            print(
                f"[BrowserServer] Unexpected error during Playwright initialization: {e}"
            )
            traceback.print_exc()
            await self.stop()
            raise

    async def _process_command(self, cmd: Command) -> Result:
        """Processes a Command object using Playwright."""
        if not self._page:
            return Result(
                success=False, error_message="Playwright page is not initialized."
            )

        try:
            if cmd.action_type == ActionType.GOTO:
                if cmd.url is None:
                    return Result(
                        success=False, error_message="Missing 'url' for goto action"
                    )
                print(f"[BrowserServer] Playwright navigating to: {cmd.url}")
                response = await self._page.goto(
                    cmd.url, wait_until="networkidle", timeout=cmd.timeout
                )
                print(f"[BrowserServer] Playwright navigation response: {response}")
                return Result(success=True)
            elif cmd.action_type == ActionType.CLICK:
                if cmd.x is None or cmd.y is None:
                    return Result(
                        success=False, error_message="Missing x or y for click action"
                    )
                print(f"[BrowserServer] Playwright clicking at: ({cmd.x}, {cmd.y})")
                await self._page.mouse.click(float(cmd.x), float(cmd.y))
                return Result(success=True)
            elif cmd.action_type == ActionType.TYPE:
                if cmd.text is None:
                    return Result(
                        success=False, error_message="Missing text for type action"
                    )
                print(
                    f"[BrowserServer] Playwright typing text (length: {len(cmd.text)})"
                )
                await self._page.keyboard.type(cmd.text, delay=20)
                return Result(success=True)
            elif cmd.action_type == ActionType.SCROLL:
                delta_x = cmd.delta_x
                delta_y = cmd.delta_y
                print(
                    f"[BrowserServer] Playwright scrolling by: dx={delta_x}, dy={delta_y}"
                )
                await self._page.mouse.wheel(delta_x, delta_y)
                return Result(success=True)
            elif cmd.action_type == ActionType.SCREENSHOT:
                print("[BrowserServer] Playwright taking screenshot")
                screenshot_bytes = await self._page.screenshot(type="jpeg", quality=80)
                return Result(success=True, image=screenshot_bytes)
            elif cmd.action_type == ActionType.SET_VIEWPORT_SIZE:
                if cmd.width is None or cmd.height is None:
                    return Result(
                        success=False,
                        error_message="Missing width or height for set_viewport_size action",
                    )
                size = {"width": int(cmd.width), "height": int(cmd.height)}
                print(f"[BrowserServer] Playwright setting viewport size to: {size}")
                await self._page.set_viewport_size(size)
                return Result(success=True)
            else:
                return Result(
                    success=False,
                    error_message=f"Unsupported action type for Playwright: {cmd.action_type.name}",
                )

        except PlaywrightError as e:
            print(
                f"[BrowserServer] Playwright Error processing action {cmd.action_type.name}: {e}"
            )
            traceback.print_exc()
            return Result(
                success=False,
                error_message=f"Playwright Error: {getattr(e, 'message', str(e))}",
            )
        except Exception as e:
            print(
                f"[BrowserServer] Error processing action {cmd.action_type.name}: {e}"
            )
            traceback.print_exc()
            return Result(
                success=False, error_message=f"Server execution error: {str(e)}"
            )

    async def handle_connection(self, websocket):
        """Handles a single WebSocket client connection."""
        peer_name = websocket.remote_address
        print(f"[BrowserServer] Client connected from {peer_name}")
        try:
            async for message_bytes in websocket:
                print(
                    f"[BrowserServer] Received {len(message_bytes)} bytes from {peer_name}"
                )
                result_obj = Result(
                    success=False, error_message="Failed to process command"
                )

                try:
                    cmd = Command.load(message_bytes)
                    # Update logging to only show non-None params and omit if empty
                    params_dict = {
                        k: v
                        for k, v in cmd.__dict__.items()
                        if k != "action_type" and v is not None
                    }
                    log_message = (
                        f"[BrowserServer] Parsed action: {cmd.action_type.name}"
                    )
                    if params_dict:
                        log_message += f" with params: {params_dict}"
                    print(log_message)
                    result_obj = await self._process_command(cmd)

                except ValueError as e:
                    print(
                        f"[BrowserServer] Error loading command/action from bytes: {e}"
                    )
                    result_obj = Result(
                        success=False,
                        error_message=f"Action deserialization error: {e}",
                    )
                except Exception as e:
                    print(f"[BrowserServer] Unexpected error handling message: {e}")
                    traceback.print_exc()
                    result_obj = Result(
                        success=False, error_message=f"Internal server error: {e}"
                    )

                try:
                    response_bytes = result_obj.dump()
                    await websocket.send(response_bytes)
                    print(
                        f"[BrowserServer] Sent response to {peer_name}: Success={result_obj.success}, HasImage={result_obj.image is not None}"
                    )
                except Exception as e:
                    print(f"[BrowserServer] Error sending response to {peer_name}: {e}")
                    break

        except websockets.exceptions.ConnectionClosedOK:
            print(f"[BrowserServer] Client {peer_name} disconnected normally.")
        except websockets.exceptions.ConnectionClosedError as e:
            print(
                f"[BrowserServer] Client {peer_name} connection closed with error: {e}"
            )
        except Exception as e:
            print(
                f"[BrowserServer] An error occurred in the connection handler for {peer_name}: {e}"
            )
            traceback.print_exc()
        finally:
            print(f"[BrowserServer] Connection closed for {peer_name}")

    async def start(self):
        """Initializes Playwright and starts the WebSocket server."""
        server = None
        try:
            await self._initialize_playwright()
            print(f"[BrowserServer] Starting WebSocket server on port {self.port}...")
            server = await websockets.serve(
                self.handle_connection,
                "0.0.0.0",
                self.port,
                max_size=2**24,
                ping_interval=20,
                ping_timeout=20,
            )
            print("[BrowserServer] WebSocket server running.")
            await self._stop_event.wait()
        except Exception as e:
            print(f"[BrowserServer] Failed to start server: {e}")
            traceback.print_exc()
            # Ensure cleanup happens even if start fails after partial initialization
            await self.stop()
        finally:
            print("[BrowserServer] Server shutdown initiated...")
            if server:
                server.close()
                await server.wait_closed()
                print("[BrowserServer] WebSocket server stopped.")
            await self.stop()

    def trigger_stop(self):
        """Signals the server to stop."""
        print("[BrowserServer] Stop signal received.")
        self._stop_event.set()

    async def stop(self):
        """Stops the Playwright resources."""
        print("[BrowserServer] Stopping Playwright resources...")
        if self._playwright_instance:
            if self._page:
                try:
                    await self._page.close()
                    print("[BrowserServer] Playwright Page closed.")
                except Exception as e:
                    print(f"[BrowserServer] Error closing page: {e}")
            if self._context:
                try:
                    await self._context.close()
                    print("[BrowserServer] Playwright Context closed.")
                except Exception as e:
                    print(f"[BrowserServer] Error closing context: {e}")
            if self._browser:
                try:
                    await self._browser.close()
                    print("[BrowserServer] Playwright Browser closed.")
                except Exception as e:
                    print(f"[BrowserServer] Error closing browser: {e}")
            try:
                await self._playwright_instance.stop()
                print("[BrowserServer] Playwright instance stopped.")
            except Exception as e:
                print(f"[BrowserServer] Error stopping Playwright instance: {e}")
            self._page, self._context, self._browser, self._playwright_instance = (
                None,
                None,
                None,
                None,
            )
        else:
            print("[BrowserServer] No Playwright resources to stop.")


async def main_async(args):
    """Asynchronous main function to manage the server lifecycle."""

    server_instance = BrowserServer(
        port=args.port,
        cdp_url=args.cdp_url,
    )
    server_type = "BrowserServer (Playwright CDP)"

    print(f"[Main] Initialized {server_type}")

    loop = asyncio.get_running_loop()
    stop_signal_received = asyncio.Event()

    def signal_handler():
        print(
            f"[{server_type}] Termination signal received. Initiating graceful shutdown..."
        )
        stop_signal_received.set()

    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            loop.add_signal_handler(sig, signal_handler)
    except NotImplementedError:
        print(
            f"[{server_type}] Warning: Signal handlers for graceful shutdown not fully supported on this platform."
        )

    server_task = asyncio.create_task(server_instance.start())

    # Wait for either the server task to complete (e.g., on error) or a stop signal
    stop_wait_task = asyncio.create_task(stop_signal_received.wait())
    done, pending = await asyncio.wait(
        [server_task, stop_wait_task], return_when=asyncio.FIRST_COMPLETED
    )

    if stop_signal_received.is_set():
        print(f"[{server_type}] Stop signal processed. Triggering server stop...")
        server_instance.trigger_stop()
        await server_task
    else:
        print(
            f"[{server_type}] Server task completed unexpectedly. Ensuring resources are stopped."
        )
        stop_wait_task.cancel()
        await server_instance.stop()

    print(f"[{server_type}] Main async function finished.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="TzafonWright WebSocket Server")
    parser.add_argument(
        "--port", type=int, default=1337, help="Port to run the WebSocket server on"
    )
    parser.add_argument(
        "--cdp-url",
        type=str,
        required=True,
        help="[Playwright only] WebSocket URL of an existing Chrome DevTools Protocol endpoint to connect to.",
    )
    args = parser.parse_args()

    print("Starting Playwright server (connecting via CDP)...")
    print(f"  Port: {args.port}")
    print(f"  CDP URL: {args.cdp_url}")

    try:
        asyncio.run(main_async(args))
    except ImportError as e:
        print(f"Error: Missing dependency - {e}")
        print(
            "Please ensure required packages ('websockets', 'playwright') are installed."
        )
    except KeyboardInterrupt:
        print("[Main] KeyboardInterrupt caught. Exiting.")
    except Exception as e:
        print(f"[Main] An unexpected error occurred: {e}")
        traceback.print_exc()
    finally:
        print("[Main] Application exiting.")
