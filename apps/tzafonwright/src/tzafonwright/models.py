import json
from enum import Enum
from dataclasses import dataclass, asdict
from typing import Any, Optional, Dict, Self, Tuple
from pydantic import BaseModel
import base64


class ActionType(Enum):
    """Types of actions that can be performed on a browser or VM"""

    CLICK = "click"
    TYPE = "type"
    SCROLL = "scroll"
    GOTO = "goto"
    SCREENSHOT = "screenshot"
    SET_VIEWPORT_SIZE = "set_viewport_size"


class ParsedAction(BaseModel):
    action: ActionType
    coordinates: Optional[Tuple[int, int]] = None
    drag_coordinates: Optional[Tuple[int, int, int, int]] = None
    text: Optional[str] = None
    direction: Optional[str] = None
    hotkey: Optional[str] = None

    def to_dict(self) -> Dict[str, Any]:
        """Convert the action to a dictionary, excluding None values"""
        result = {
            "action": self.action.value,
        }

        if self.coordinates is not None:
            result["coordinates"] = self.coordinates
        if self.drag_coordinates is not None:
            result["drag_coordinates"] = self.drag_coordinates
        if self.text is not None:
            result["text"] = self.text
        if self.direction is not None:
            result["direction"] = self.direction
        if self.hotkey is not None:
            result["hotkey"] = self.hotkey

        return result


@dataclass
class Command:
    action_type: ActionType
    x: Optional[float] = None
    y: Optional[float] = None
    text: Optional[str] = None
    delta_x: Optional[int] = None
    delta_y: Optional[int] = None
    url: Optional[str] = None
    width: Optional[int] = None
    height: Optional[int] = None
    timeout: int = 30000

    @classmethod
    def load(cls, message_bytes: bytes) -> Self:
        """Loads a Command object from JSON bytes."""
        try:
            data = json.loads(message_bytes.decode("utf-8"))
            action_type_str = data.pop("action_type", None)
            if not action_type_str:
                raise ValueError("Missing 'action_type' in message")
            try:
                action_type_enum = ActionType(action_type_str)
            except ValueError:
                raise ValueError(f"Invalid action_type: {action_type_str}")
            valid_keys = cls.__dataclass_fields__.keys()
            filtered_data = {
                k: v for k, v in data.items() if k in valid_keys and k != "action_type"
            }
            return cls(action_type=action_type_enum, **filtered_data)
        except json.JSONDecodeError as e:
            raise ValueError(f"Failed to decode JSON: {e}") from e
        except Exception as e:
            raise ValueError(f"Error creating Command object: {e}") from e

    def dump(self) -> bytes:
        """Dumps the Command object to JSON bytes, excluding None values."""
        data = asdict(self)
        data["action_type"] = self.action_type.value
        filtered_data = {k: v for k, v in data.items() if v is not None}
        return json.dumps(filtered_data).encode("utf-8")


@dataclass
class Result:
    """Standard result object for command execution."""

    success: bool
    image: Optional[bytes] = None
    error_message: Optional[str] = None

    @classmethod
    def load(cls, message_bytes: bytes) -> Self:
        """Loads a Result object from JSON bytes."""
        try:
            data = json.loads(message_bytes.decode("utf-8"))
            success = data.get("success", False)
            error_message = data.get("error_message")
            image_b64 = data.get("image")
            image = None
            if image_b64:
                try:
                    image = base64.b64decode(image_b64)
                except (TypeError, ValueError) as e:
                    raise ValueError(f"Failed to decode base64 image data: {e}") from e

            return cls(success=success, image=image, error_message=error_message)
        except json.JSONDecodeError as e:
            raise ValueError(f"Failed to decode JSON for Result: {e}") from e
        except Exception as e:
            raise ValueError(f"Error creating Result object: {e}") from e

    def dump(self) -> bytes:
        """Dumps the Result object to JSON bytes, encoding image to base64."""
        data = {
            "success": self.success,
            "error_message": self.error_message,
            "image": None,
        }
        if self.image:
            data["image"] = base64.b64encode(self.image).decode("utf-8")
        filtered_data = {k: v for k, v in data.items() if v is not None}
        return json.dumps(filtered_data).encode("utf-8")

    def __str__(self) -> str:
        status = "Success" if self.success else "Error"
        img_status = "[Image Present]" if self.image else "[No Image]"
        err_msg = f", Error: {self.error_message}" if self.error_message else ""
        return f"Result(status={status}, image_status={img_status}{err_msg})"
