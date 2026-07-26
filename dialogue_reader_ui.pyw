"""Launcher for the DialogReader settings app (double-click me).
.pyw = no console window."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from ui.app import run

run()
