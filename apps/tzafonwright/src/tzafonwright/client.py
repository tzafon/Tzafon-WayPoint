import asyncio
import traceback
from typing import Optional

import websockets
import websockets.asyncio.client
from websockets.connection import State

from tzafonwright.models import Result, Command


class TzafonWrightClient:
    """Client for TzafonWright automation tasks via WebSocket."""

    def __init__(self, ws_url: str):
        """Initializes the client with the WebSocket server URL."""
        if not ws_url or not ws_url.startswith(("ws://", "wss://")):
            raise ValueError(f"Invalid WebSocket URL provided: {ws_url}")
        self.ws_url = ws_url
        self.connection: Optional[websockets.asyncio.client.ClientConnection] = None

    async def __aenter__(self):
        await self.connect()
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        await self.close()

    async def connect(self):
        """Connect to the TzafonWright WebSocket server."""
        if self.is_connected:
            print("[Client] Already connected.")
            return self

        print(f"[Client] Attempting WebSocket connection to {self.ws_url}")
        try:
            self.connection = await websockets.connect(
                self.ws_url,
                max_size=2**24,  # 16MB limit
                ping_interval=20,
                ping_timeout=20,
            )
            print(f"[Client] Connected to TzafonWright server at {self.ws_url}.")
        except Exception as e:
            print(f"[Client] Error: WebSocket connection failed to {self.ws_url}: {e}")
            traceback.print_exc()
            self.connection = None
            raise
        return self

    async def send_action(self, cmd: Command) -> Result:
        """Sends an action to the server and returns the Result."""
        if not self.is_connected or not self.connection:
            return Result(success=False, error_message="Client not connected.")

        try:
            command_bytes = cmd.dump()

            # Update logging to use filtered args and omit if empty
            log_message = f"[Client] Sending command: {cmd}"
            print(log_message)

            await self.connection.send(command_bytes)

            print("[Client] Waiting for response...")  # Logging
            response_message = await self.connection.recv()
            print(
                f"[Client] Received response (type: {type(response_message)})"
            )  # Logging

            if isinstance(response_message, bytes):
                response_bytes = response_message
            elif isinstance(response_message, str):
                print(
                    "[Client] Warning: Received string message, expected bytes. Encoding as UTF-8."
                )
                response_bytes = response_message.encode("utf-8")
            else:
                raise ValueError(
                    f"Received unexpected message type from server: {type(response_message)}"
                )

            result_obj = Result.load(response_bytes)
            print(f"[Client] Parsed result: {result_obj}")  # Logging
            return result_obj

        except websockets.ConnectionClosed as e:
            print(f"[Client] Error: WebSocket connection closed during send/recv: {e}")
            traceback.print_exc()
            await self.close()
            return Result(
                success=False,
                error_message=f"WebSocket connection closed unexpectedly: {e.reason} (Code: {e.code})",
            )
        except (ValueError, TypeError) as e:
            print(f"[Client] Error processing data for action {cmd}: {e}")
            traceback.print_exc()
            return Result(success=False, error_message=f"Data Error: {e}")
        except Exception as e:
            print(f"[Client] Unexpected error processing action {cmd}: {e}")
            traceback.print_exc()
            return Result(
                success=False, error_message=f"Unexpected client error: {str(e)}"
            )

    async def close(self):
        """Safely closes the WebSocket connection."""
        conn = self.connection
        current_state_name = "Unknown"
        is_open_state = False
        if conn:
            try:
                current_state = conn.state
                current_state_name = current_state.name
                is_open_state = current_state == State.OPEN
            except Exception:
                current_state_name = "ErrorAccessingState"

        print(
            f"[Client] close() called. Connection object: {'Exists' if conn else 'None'}, State: {current_state_name}"
        )

        try:
            if conn and is_open_state:
                print(
                    f"[Client] Attempting to close WebSocket connection to {self.ws_url} (State: {current_state_name})..."
                )
                try:
                    await conn.close(code=1000, reason="Client closing")

                    await asyncio.wait_for(conn.wait_closed(), timeout=5.0)
                    print("[Client] WebSocket connection closed.")
                except asyncio.TimeoutError:
                    print(
                        "[Client] Warning: Timeout waiting for WebSocket connection to close."
                    )

                except Exception as e:
                    print(f"[Client] Error closing WebSocket connection: {e}")
                    traceback.print_exc()
            elif conn and not is_open_state:
                print(
                    f"[Client] Info: Connection already closed or closing (State: {current_state_name}). No close action needed."
                )

            self.connection = None
        except Exception as e:
            print(f"[Client] Error during close(): {e}")
            traceback.print_exc()
            self.connection = None

    @property
    def is_connected(self) -> bool:
        """Check if the client has an active WebSocket connection (State is OPEN)."""
        conn = self.connection
        if conn is None:
            return False

        try:
            current_state = conn.state
            is_conn = current_state == State.OPEN
            return is_conn
        except Exception as e:
            print(f"[Client] is_connected check: Error accessing connection state: {e}")
            self.connection = None
            return False


async def connect_tzafonwright(ws_url: str) -> TzafonWrightClient:
    """Connect to the TzafonWright server using TzafonWrightClient."""
    client = TzafonWrightClient(ws_url)
    await client.connect()
    return client
