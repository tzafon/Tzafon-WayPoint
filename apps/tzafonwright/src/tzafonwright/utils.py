from typing import Optional, Callable
from tzafonwright.models import ActionType, ParsedAction

# --- Action Handler Constants ---
DEFAULT_SCROLL_AMOUNT = 100

# --- Action Handler Type Alias ---
ActionHandler = Callable[[ParsedAction], Optional[dict]]


# --- Action Handler Functions ---
def handle_click(action: ParsedAction) -> Optional[dict]:
    """Generates the action dictionary for a CLICK action."""
    if action.coordinates:
        return {
            "action_type": ActionType.CLICK.value,
            "x": action.coordinates[0],
            "y": action.coordinates[1],
        }
    else:
        print("[Warning] CLICK action missing coordinates. Skipping.")
        return None


def handle_type(action: ParsedAction) -> Optional[dict]:
    """Generates the action dictionary for a TYPE action."""
    if action.text is not None:
        # Basic sanitization - could be expanded
        sanitized_text = action.text.replace("\n", " ").replace("\r", "")
        if not sanitized_text:
            print("[Warning] TYPE action has empty text after sanitization. Skipping.")
            return None
        return {"action_type": ActionType.TYPE.value, "text": sanitized_text}
    else:
        print("[Warning] TYPE action missing text. Skipping.")
        return None


def handle_scroll(action: ParsedAction) -> Optional[dict]:
    """Generates the action dictionary for a SCROLL action."""
    delta_x = 0
    delta_y = 0
    direction = getattr(action, "direction", "down")  # Default to down if missing

    if direction == "down":
        delta_y = DEFAULT_SCROLL_AMOUNT
    elif direction == "up":
        delta_y = -DEFAULT_SCROLL_AMOUNT
    elif direction == "left":
        delta_x = -DEFAULT_SCROLL_AMOUNT
    elif direction == "right":
        delta_x = DEFAULT_SCROLL_AMOUNT
    else:
        print(
            f"[Warning] SCROLL action has unknown direction '{direction}', defaulting to scroll down."
        )
        delta_y = DEFAULT_SCROLL_AMOUNT

    return {
        "action_type": ActionType.SCROLL.value,
        "delta_x": delta_x,
        "delta_y": delta_y,
    }


def handle_goto(action: ParsedAction) -> Optional[dict]:
    """Generates the action dictionary for a GOTO action."""
    if action.url:
        return {"action_type": ActionType.GOTO.value, "url": action.url}
    else:
        print("[Warning] GOTO action missing URL. Skipping.")
        return None
