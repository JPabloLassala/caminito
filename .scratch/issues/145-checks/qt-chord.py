"""Ticket 145: a Qt window whose File menu has a shortcut of one press, two chords and a
shortcut of four presses, the most Qt takes. Run in the session, through 119-checks/in.sh.
It needs pyside6."""
import sys

from PySide6.QtGui import QAction, QKeySequence
from PySide6.QtWidgets import QApplication, QMainWindow

app = QApplication(sys.argv)
window = QMainWindow()
window.setWindowTitle("A Qt app with chords")
menu = window.menuBar().addMenu("&File")
for label, keys in [("&Open...", "Ctrl+O"), ("Open &Folder...", "Ctrl+K, Ctrl+O"),
                    ("&Close Folder", "Ctrl+K, F"), ("Four &Presses", "Ctrl+X, Ctrl+K, Alt+F4, Q")]:
    action = QAction(label, window)
    action.setShortcut(QKeySequence(keys))
    menu.addAction(action)
window.resize(400, 200)
window.show()
sys.exit(app.exec())
