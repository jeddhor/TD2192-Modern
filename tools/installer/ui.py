"""Qt 6 installer/uninstaller front end; all filesystem work lives in sibling core.py.

Run directly, as ``python -m tools.installer.ui``, or as a PyInstaller one-file
executable. ``--install`` and ``--quiet`` delegate to core's noninteractive CLI.
The GUI never removes saves/configuration and never runs core work on the UI thread.
"""
from __future__ import annotations

import argparse
import importlib
import sys
import zipfile
from pathlib import Path
from typing import Callable

from PySide6.QtCore import QThread, QTimer, Qt, Signal, Slot
from PySide6.QtGui import QColor, QCloseEvent, QPalette
from PySide6.QtWidgets import (
    QApplication, QCheckBox, QDialog, QDialogButtonBox, QFileDialog, QHBoxLayout,
    QLabel, QLineEdit, QMessageBox, QPlainTextEdit, QProgressBar, QPushButton,
    QVBoxLayout, QWizard, QWizardPage,
)

APP_NAME = "TD2192-Modern"
WELCOME, DESTINATION, RETAIL, SHORTCUTS, REVIEW, PROGRESS, RESULT = range(7)


def _core_module():
    return importlib.import_module(f"{__package__}.core" if __package__ else "core")


def _label(text: str = "", *, muted: bool = False) -> QLabel:
    label = QLabel(text)
    label.setTextFormat(Qt.TextFormat.PlainText)
    label.setWordWrap(True)
    if muted:
        label.setObjectName("muted")
    return label


def _path(text: str) -> Path:
    return Path(text.strip()).expanduser().absolute()


def _is_notice_text(name: str) -> bool:
    """License/archive display is text only; never decode source ZIPs or image binaries."""
    path = Path(name)
    suffix = path.suffix.casefold()
    if suffix in {".txt", ".text", ".md", ".rst", ".json", ".html", ".htm", ".license"}:
        return True
    if suffix.startswith((".gpl", ".lgpl", ".agpl", ".bsd", ".mit", ".apache")):
        return True
    return not suffix and path.name.casefold().startswith(
        ("license", "licence", "copying", "notice", "copyright", "readme", "authors", "gpl", "lgpl", "agpl"))


def _read_license_notices(directory: Path) -> str:
    """Read plain notices and the one bundled license archive without extracting anything."""
    sections = []
    total_bytes = 0

    def add(name, data):
        nonlocal total_bytes
        total_bytes += len(data)
        if len(data) > 8 * 1024 * 1024 or total_bytes > 32 * 1024 * 1024:
            raise ValueError("Bundled license text exceeds the viewer's size limit.")
        sections.append(f"{name}\n{'─' * 60}\n" + data.decode("utf-8", errors="replace"))

    files = sorted((path for path in directory.rglob("*")
                    if path.is_file() and _is_notice_text(path.name)),
                   key=lambda path: (path.name.upper() != "NOTICE.TXT", str(path).lower()))
    for path in files:
        if path.stat().st_size > 8 * 1024 * 1024:
            raise ValueError("Bundled license text exceeds the viewer's size limit.")
        add(str(path.relative_to(directory)), path.read_bytes())
    archive = directory / "LICENSES.zip"
    if archive.is_file():
        with zipfile.ZipFile(archive) as licenses:
            for entry in sorted(licenses.infolist(), key=lambda info: info.filename.casefold()):
                if entry.is_dir() or not _is_notice_text(entry.filename):
                    continue
                if entry.file_size > 8 * 1024 * 1024 or total_bytes + entry.file_size > 32 * 1024 * 1024:
                    raise ValueError("Bundled license text exceeds the viewer's size limit.")
                add(f"LICENSES.zip / {entry.filename}", licenses.read(entry))
    if not sections:
        raise FileNotFoundError("The bundled installer license notices could not be found.")
    return "\n\n".join(sections)


class CoreTask(QThread):
    """Run one blocking core operation, reporting only queued signals to widgets."""
    progressed = Signal(str, int)
    succeeded = Signal(object)
    failed = Signal(str)

    def __init__(self, action: Callable, parent=None):
        super().__init__(parent)
        self.action = action

    def run(self):
        try:
            result = self.action(self.progressed.emit)
        except Exception as error:
            self.failed.emit(str(error) or error.__class__.__name__)
        else:
            self.succeeded.emit(result)


class Page(QWizardPage):
    @property
    def installer(self) -> "InstallerWizard":
        return self.wizard()

    def nextId(self):
        page_id = self.installer.currentId()
        if self.installer.uninstalling:
            return {WELCOME: DESTINATION, DESTINATION: REVIEW, REVIEW: PROGRESS,
                    PROGRESS: RESULT}.get(page_id, -1)
        return {WELCOME: DESTINATION, DESTINATION: RETAIL, RETAIL: SHORTCUTS,
                SHORTCUTS: REVIEW, REVIEW: PROGRESS, PROGRESS: RESULT}.get(page_id, -1)


class WelcomePage(Page):
    def __init__(self, uninstalling: bool):
        super().__init__()
        self.setTitle("Uninstall TD2192-Modern" if uninstalling else "Welcome to TD2192-Modern")
        self.setSubTitle("Traffic Department 2192 · Modern edition")
        layout = QVBoxLayout(self)
        badge = _label("UNINSTALL" if uninstalling else "SETUP")
        badge.setObjectName("eyebrow")
        layout.addWidget(badge)
        if uninstalling:
            layout.addWidget(_label("Remove the installed game and its shortcuts."))
            layout.addWidget(_label("Your saved games and settings will be kept. Your original retail game folder will not be changed."))
            layout.addWidget(_label("Close the game before continuing.", muted=True))
        else:
            layout.addWidget(_label("Play the original Traffic Department 2192 on a modern computer, with all three episodes and optional visual enhancements."))
            layout.addWidget(_label("You will choose an installation folder, import the required files from your own retail game folder, and choose your shortcuts."))
            layout.addWidget(_label("No game data is included in this installer. Your original files are read only; setup copies the files it needs.", muted=True))
            layout.addWidget(_label("Installs for your user account. Administrator access is not required.", muted=True))
        self.licenses_button = QPushButton("Licenses…")
        self.licenses_button.clicked.connect(lambda: self.installer.show_licenses())
        layout.addWidget(self.licenses_button, alignment=Qt.AlignmentFlag.AlignLeft)
        layout.addStretch(1)


class DestinationPage(Page):
    def __init__(self, target: Path, uninstalling: bool):
        super().__init__()
        self.setTitle("Choose the installed game" if uninstalling else "Choose where to install")
        self.setSubTitle("Saved games and settings are kept separately from this folder." if uninstalling
                         else "Install for your account, in a folder you can write to.")
        layout = QVBoxLayout(self)
        self.path_edit = QLineEdit(str(target))
        self.path_edit.setAccessibleName("Installed game folder" if uninstalling else "Installation folder")
        self.path_edit.setClearButtonEnabled(True)
        browse = QPushButton("Browse…")
        browse.clicked.connect(self.browse)
        row = QHBoxLayout()
        row.addWidget(self.path_edit, 1)
        row.addWidget(browse)
        layout.addWidget(_label("Installed game folder" if uninstalling else "Installation folder"))
        layout.addLayout(row)
        self.error = _label()
        self.error.setObjectName("error")
        layout.addWidget(self.error)
        layout.addWidget(_label("Setup will check this folder before making changes.", muted=True))
        layout.addStretch(1)
        self.path_edit.textChanged.connect(self.completeChanged)

    def browse(self):
        selected = QFileDialog.getExistingDirectory(self, "Choose a folder", self.path_edit.text())
        if selected:
            self.path_edit.setText(selected)

    def isComplete(self):
        return bool(self.path_edit.text().strip())

    def validatePage(self):
        try:
            target = _path(self.path_edit.text())
            if target == Path(target.anchor):
                raise ValueError("Choose a game folder, rather than the root of a drive.")
        except (ValueError, OSError, RuntimeError) as error:
            self.error.setText(str(error))
            return False
        self.error.clear()
        self.installer.target = target
        return True


class RetailPage(Page):
    def __init__(self, source: Path | None):
        super().__init__()
        self.setTitle("Bring your original game files")
        self.setSubTitle("Choose the local retail game folder that contains FILE0001.EXE.")
        self.validated = ""
        layout = QVBoxLayout(self)
        layout.addWidget(_label("Your original game folder stays where it is. Only the required game files are copied into the installation."))
        self.path_edit = QLineEdit(str(source) if source else "")
        self.path_edit.setAccessibleName("Original retail game folder")
        self.path_edit.setPlaceholderText("Choose your Traffic Department 2192 folder")
        self.browse_button = QPushButton("Browse…")
        self.browse_button.clicked.connect(self.browse)
        row = QHBoxLayout()
        row.addWidget(self.path_edit, 1)
        row.addWidget(self.browse_button)
        layout.addLayout(row)
        self.check_button = QPushButton("Check game files")
        self.check_button.clicked.connect(self.check_source)
        layout.addWidget(self.check_button, alignment=Qt.AlignmentFlag.AlignLeft)
        self.status = _label("Choose a folder, then check the game files before continuing.", muted=True)
        layout.addWidget(self.status)
        layout.addStretch(1)
        self.path_edit.textChanged.connect(self.source_changed)

    def source_changed(self):
        self.validated = ""
        self.status.setObjectName("muted")
        self.status.setText("Check the game files before continuing.")
        self.completeChanged.emit()

    def browse(self):
        selected = QFileDialog.getExistingDirectory(self, "Find the original game folder", self.path_edit.text())
        if selected:
            self.path_edit.setText(selected)
            self.check_source()

    def isComplete(self):
        return bool(self.validated) and self.validated == self.path_edit.text().strip()

    def check_source(self):
        text = self.path_edit.text().strip()
        if not text or self.installer.busy:
            return
        try:
            source = _path(text)
        except (ValueError, OSError, RuntimeError) as error:
            self.status.setText(f"Choose a usable game folder. {error}")
            return
        self.status.setText("Checking required game files…")
        self.check_button.setEnabled(False)
        self.browse_button.setEnabled(False)
        self.path_edit.setEnabled(False)

        def finished(result):
            self.check_button.setEnabled(True)
            self.browse_button.setEnabled(True)
            self.path_edit.setEnabled(True)
            self.validated = text
            self.installer.source = source
            self.status.setText(f"Ready — {len(result)} required game files found.")
            self.completeChanged.emit()

        def failed(message):
            self.check_button.setEnabled(True)
            self.browse_button.setEnabled(True)
            self.path_edit.setEnabled(True)
            self.validated = ""
            self.status.setText(f"The folder could not be used. {message}")
            self.completeChanged.emit()

        self.installer.start_task(lambda progress: self.installer.core.validate_source(source), finished, failed)


class ShortcutsPage(Page):
    def __init__(self, start_menu: bool, desktop: bool):
        super().__init__()
        self.setTitle("Make it easy to find")
        self.setSubTitle("Choose shortcuts for your user account.")
        layout = QVBoxLayout(self)
        self.start_menu = QCheckBox("Add a Start menu shortcut" if sys.platform == "win32"
                                   else "Add an Applications menu entry")
        self.desktop = QCheckBox("Add a desktop shortcut")
        self.start_menu.setChecked(start_menu)
        self.desktop.setChecked(desktop)
        layout.addWidget(self.start_menu)
        layout.addWidget(self.desktop)
        layout.addWidget(_label("You can also launch the game directly from its installation folder.", muted=True))
        layout.addStretch(1)


class ReviewPage(Page):
    def __init__(self, uninstalling: bool):
        super().__init__()
        self.setTitle("Ready to uninstall" if uninstalling else "Ready to install")
        self.setSubTitle("Review these choices before continuing.")
        self.setCommitPage(True)
        layout = QVBoxLayout(self)
        self.summary = _label()
        self.summary.setTextInteractionFlags(Qt.TextInteractionFlag.TextSelectableByMouse)
        layout.addWidget(self.summary)
        layout.addStretch(1)

    def initializePage(self):
        wizard = self.installer
        if wizard.uninstalling:
            self.summary.setText(f"Remove TD2192-Modern from\n{wizard.target}\n\nInstalled game files and shortcuts will be removed.\nYour saved games, settings and original retail folder will be kept.")
        else:
            shortcuts = []
            if wizard.shortcuts_page.start_menu.isChecked():
                shortcuts.append("Start menu" if sys.platform == "win32" else "Applications menu")
            if wizard.shortcuts_page.desktop.isChecked():
                shortcuts.append("Desktop")
            self.summary.setText(f"Install in\n{wizard.target}\n\nCopy required game files from\n{wizard.source}\n\nShortcuts: {', '.join(shortcuts) if shortcuts else 'None'}\n\nYour original game folder will not be changed.")


class ProgressPage(Page):
    def __init__(self, uninstalling: bool):
        super().__init__()
        self.setTitle("Removing TD2192-Modern" if uninstalling else "Installing TD2192-Modern")
        self.setSubTitle("Please wait while setup finishes. This window will stay responsive.")
        self.done = False
        layout = QVBoxLayout(self)
        self.status = _label("Preparing…")
        self.bar = QProgressBar()
        self.bar.setRange(0, 100)
        self.log = QPlainTextEdit()
        self.log.setReadOnly(True)
        self.log.setAccessibleName("Installation progress details")
        self.log.document().setMaximumBlockCount(400)
        layout.addWidget(self.status)
        layout.addWidget(self.bar)
        layout.addWidget(self.log, 1)

    def initializePage(self):
        self.done = False
        self.log.clear()
        self.bar.setValue(0)
        # Defer until the progress page and navigation state are painted.
        QTimer.singleShot(0, self.installer.begin_operation)

    def isComplete(self):
        return self.done

    @Slot(str, int)
    def update_progress(self, message: str, percent: int):
        self.status.setText(message)
        self.bar.setValue(max(0, min(100, percent)))
        self.log.appendPlainText(message)


class ResultPage(Page):
    def __init__(self):
        super().__init__()
        self.setFinalPage(True)
        layout = QVBoxLayout(self)
        self.summary = _label()
        self.launch = QCheckBox("Launch TD2192-Modern")
        self.launch.setChecked(True)
        self.retry = QPushButton("Back to setup")
        self.retry.clicked.connect(lambda: self.installer.retry())
        self.details = QPlainTextEdit()
        self.details.setReadOnly(True)
        self.details.setAccessibleName("Setup result details")
        self.details.setMaximumHeight(160)
        layout.addWidget(self.summary)
        layout.addWidget(self.launch)
        layout.addWidget(self.retry, alignment=Qt.AlignmentFlag.AlignLeft)
        layout.addWidget(self.details)
        layout.addStretch(1)

    def initializePage(self):
        wizard = self.installer
        self.setTitle("TD2192-Modern was removed" if wizard.success and wizard.uninstalling
                      else "You're ready to play" if wizard.success else "Setup could not finish")
        self.setSubTitle("Your saved games and settings have been kept." if wizard.uninstalling
                         else "You can close setup when you are ready.")
        self.launch.setVisible(wizard.success and not wizard.uninstalling)
        self.retry.setVisible(not wizard.success)
        self.details.setVisible(not wizard.success)
        if wizard.success:
            self.summary.setText("The installed game and shortcuts have been removed." if wizard.uninstalling
                                 else f"TD2192-Modern is installed in\n{wizard.target}")
            if wizard.modified_assets:
                count = len(wizard.modified_assets)
                files = "file differs" if count == 1 else "files differ"
                self.summary.setText(self.summary.text() + f"\n\n{count} imported {files} from reference retail data; modified files were preserved.")
        else:
            self.summary.setText(wizard.error)
            self.details.setPlainText(wizard.progress_page.log.toPlainText())
        wizard.setButtonText(QWizard.WizardButton.FinishButton, "Finish" if wizard.success else "Close")


class InstallerWizard(QWizard):
    """Installation flow. Inject core_api to test Qt behavior without touching real files."""
    def __init__(self, *, uninstalling=False, target=None, source=None, start_menu=True,
                 desktop=False, core_api=None):
        super().__init__()
        self.core = core_api if core_api is not None else _core_module()
        self.uninstalling = uninstalling
        self.target = Path(target) if target else Path(self.core.default_target())
        self.source = Path(source) if source else None
        self.success = False
        self.error = ""
        self.busy = False
        self.exit_code = 1
        self._worker = None
        self.modified_assets = []
        self._task_result = None
        self._task_error = None
        self._on_success = None
        self._on_failure = None
        self.setWindowTitle(f"{APP_NAME} — {'Uninstall' if uninstalling else 'Setup'}")
        self.setWizardStyle(QWizard.WizardStyle.ModernStyle)
        self.setMinimumSize(660, 480)
        self.resize(760, 560)
        self.setOption(QWizard.WizardOption.NoBackButtonOnLastPage, True)
        self.setOption(QWizard.WizardOption.NoCancelButtonOnLastPage, True)
        self.setButtonText(QWizard.WizardButton.NextButton, "Continue")
        self.setButtonText(QWizard.WizardButton.CommitButton, "Uninstall" if uninstalling else "Install")
        self.setPage(WELCOME, WelcomePage(uninstalling))
        self.destination_page = DestinationPage(self.target, uninstalling)
        self.setPage(DESTINATION, self.destination_page)
        self.retail_page = RetailPage(self.source)
        self.setPage(RETAIL, self.retail_page)
        self.shortcuts_page = ShortcutsPage(start_menu, desktop)
        self.setPage(SHORTCUTS, self.shortcuts_page)
        self.setPage(REVIEW, ReviewPage(uninstalling))
        self.progress_page = ProgressPage(uninstalling)
        self.setPage(PROGRESS, self.progress_page)
        self.result_page = ResultPage()
        self.setPage(RESULT, self.result_page)
        self.setStyleSheet("""
            QWizard { background: #f4f6f8; color: #192a38; }
            QWizardPage { background: #f4f6f8; }
            QLabel { font-size: 10pt; }
            QLabel#muted { color: #526574; }
            QLabel#eyebrow { color: #246b85; font-size: 9pt; font-weight: 600; }
            QLabel#error { color: #a02a26; }
            QLineEdit, QPlainTextEdit { background: white; color: #192a38; border: 1px solid #b9c6ce; border-radius: 4px; padding: 7px; }
            QPushButton { padding: 6px 15px; min-height: 22px; }
            QCheckBox { spacing: 9px; padding: 7px 0; font-size: 10pt; }
            QProgressBar { border: 1px solid #b9c6ce; border-radius: 4px; background: white; text-align: center; color: #192a38; min-height: 22px; }
            QProgressBar::chunk { background: #409ab5; }
        """)

    def show_licenses(self):
        if self.busy:
            return

        def read_notices(progress):
            base = (Path(self.core.resource_root()) if hasattr(self.core, "resource_root")
                    else Path(getattr(sys, "_MEIPASS", Path(__file__).resolve().parent)))
            return _read_license_notices(base / "installer-notices")

        self.start_task(read_notices, self._show_notices,
                        lambda message: QMessageBox.warning(self, "License notices", message))

    def _show_notices(self, text):
        dialog = QDialog(self)
        dialog.setWindowTitle("Licenses and notices")
        dialog.resize(760, 560)
        layout = QVBoxLayout(dialog)
        layout.addWidget(_label("License notices included with this installer and installed with the game."))
        contents = QPlainTextEdit()
        contents.setReadOnly(True)
        contents.setAccessibleName("Installer and game license notices")
        contents.setPlainText(text)
        layout.addWidget(contents, 1)
        close = QDialogButtonBox(QDialogButtonBox.StandardButton.Close)
        close.rejected.connect(dialog.reject)
        layout.addWidget(close)
        dialog.exec()

    def start_task(self, action, on_success, on_failure, *, progress=None):
        if self.busy:
            return
        self.busy = True
        self._task_result = self._task_error = None
        self._on_success, self._on_failure = on_success, on_failure
        self._worker = CoreTask(action, self)
        self._worker.succeeded.connect(self._task_succeeded)
        self._worker.failed.connect(self._task_failed)
        self._worker.finished.connect(self._task_finished)
        if progress:
            self._worker.progressed.connect(progress)
        self._set_navigation_enabled(False)
        self._worker.start()

    def _set_navigation_enabled(self, enabled):
        for button in (QWizard.WizardButton.BackButton, QWizard.WizardButton.NextButton,
                       QWizard.WizardButton.CommitButton, QWizard.WizardButton.FinishButton,
                       QWizard.WizardButton.CancelButton):
            self.button(button).setEnabled(enabled)
        if enabled:
            self.page(self.currentId()).completeChanged.emit()

    @Slot(object)
    def _task_succeeded(self, result):
        self._task_result = result

    @Slot(str)
    def _task_failed(self, message):
        self._task_error = message

    @Slot()
    def _task_finished(self):
        worker = self._worker
        self._worker = None
        self.busy = False
        if worker:
            worker.deleteLater()
        self._set_navigation_enabled(True)
        if self._task_error is not None:
            self._on_failure(self._task_error)
        else:
            self._on_success(self._task_result)

    def begin_operation(self):
        if self.busy or self.currentId() != PROGRESS:
            return
        self.success = False
        self.error = ""
        if self.uninstalling:
            action = lambda progress: self.core.uninstall(self.target, remove_user_data=False, progress=progress)
        else:
            source, target = self.source, self.target
            menu = self.shortcuts_page.start_menu.isChecked()
            desktop = self.shortcuts_page.desktop.isChecked()
            action = lambda progress: self.core.install(source, target, start_menu=menu, desktop=desktop, progress=progress)
        self.start_task(action, self._operation_succeeded, self._operation_failed,
                        progress=self.progress_page.update_progress)

    def _operation_succeeded(self, result):
        self.modified_assets = result.get("modified_assets", []) if isinstance(result, dict) else []
        self.success = True
        self.exit_code = 0
        self.progress_page.bar.setValue(100)
        self.progress_page.done = True
        self.progress_page.completeChanged.emit()
        self.next()

    def _operation_failed(self, message):
        self.error = message
        self.exit_code = 1
        self.progress_page.log.appendPlainText(message)
        self.progress_page.done = True
        self.progress_page.completeChanged.emit()
        self.next()

    def retry(self):
        if not self.busy:
            self.success = False
            self.exit_code = 1
            self.error = ""
            self.restart()

    def accept(self):
        if self.busy:
            return
        if self.success and not self.uninstalling and self.result_page.launch.isChecked():
            self.start_task(lambda progress: self.core.launch(self.target),
                            lambda result: super(InstallerWizard, self).accept(), self._launch_failed)
        else:
            super().accept()

    def _launch_failed(self, message):
        self.result_page.launch.setChecked(False)
        self.result_page.summary.setText(f"The game was installed, but could not be started.\n{message}\n\nYou can start it later from the installation folder.")

    def reject(self):
        if not self.busy:
            super().reject()

    def closeEvent(self, event: QCloseEvent):
        if self.busy:
            event.ignore()
        else:
            super().closeEvent(event)


class GuiParser(argparse.ArgumentParser):
    def error(self, message):
        raise ValueError(message)


def main(argv=None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    core = _core_module()
    frozen = getattr(sys, "frozen", False)
    executable = Path(sys.executable if frozen else sys.argv[0])
    auto_uninstall = executable.name.lower().startswith("uninstall")
    if auto_uninstall and "--install" not in args and "--uninstall" not in args:
        args.insert(0, "--uninstall")
    if frozen and "--uninstall" in args and not any(arg == "--target" or arg.startswith("--target=") for arg in args):
        args.extend(["--target", str(executable.parent)])
    if "--install" in args or "--quiet" in args or "--help" in args or "-h" in args:
        return core.main(args)
    parser = GuiParser(description="TD2192-Modern Qt setup")
    parser.add_argument("--uninstall", action="store_true")
    parser.add_argument("--target", type=Path)
    parser.add_argument("--source", type=Path)
    parser.add_argument("--gui-smoke-test", type=Path, help=argparse.SUPPRESS)
    parser.add_argument("--start-menu", dest="start_menu", action="store_true", default=True)
    parser.add_argument("--no-start-menu", dest="start_menu", action="store_false")
    parser.add_argument("--desktop", dest="desktop", action="store_true", default=False)
    parser.add_argument("--no-desktop", dest="desktop", action="store_false")
    app = QApplication.instance() or QApplication([sys.argv[0]])
    app.setApplicationName(APP_NAME)
    app.setOrganizationName(APP_NAME)
    app.setStyle("Fusion")
    # The wizard uses light page backgrounds. An inherited OS dark palette makes
    # their body text unreadable, so use Fusion's matching light palette too.
    palette = QPalette()
    colors = {
        "Window": "#f4f6f8", "WindowText": "#192a38", "Base": "#ffffff",
        "AlternateBase": "#eef2f5", "Text": "#192a38", "Button": "#e5eaee",
        "ButtonText": "#192a38", "BrightText": "#ffffff", "Highlight": "#246b85",
        "HighlightedText": "#ffffff", "ToolTipBase": "#ffffff", "ToolTipText": "#192a38",
        "PlaceholderText": "#526574", "Light": "#ffffff", "Midlight": "#edf1f4",
        "Mid": "#b9c6ce", "Dark": "#7e8c97", "Shadow": "#526574",
    }
    for role, color in colors.items():
        palette.setColor(getattr(QPalette.ColorRole, role), QColor(color))
    for role in (QPalette.ColorRole.Text, QPalette.ColorRole.WindowText, QPalette.ColorRole.ButtonText):
        palette.setColor(QPalette.ColorGroup.Disabled, role, QColor("#7e8c97"))
    app.setPalette(palette)
    try:
        options = parser.parse_args(args)
        uninstalling = options.uninstall or auto_uninstall
        target = options.target
        if uninstalling and target is None:
            if getattr(sys, "frozen", False):
                target = Path(sys.executable).parent
            else:
                raise ValueError("Uninstalling from Python requires --target with the installed game folder.")
        source = options.source
        if source is None and not uninstalling:
            source = core.default_source()
        wizard = InstallerWizard(uninstalling=uninstalling, target=target, source=source,
                                 start_menu=options.start_menu, desktop=options.desktop, core_api=core)
    except (ValueError, OSError) as error:
        QMessageBox.critical(None, f"{APP_NAME} setup", str(error))
        return 2
    smoke_exit = [1]
    if options.gui_smoke_test:
        def capture_smoke():
            try:
                path = options.gui_smoke_test
                path.parent.mkdir(parents=True, exist_ok=True)
                smoke_exit[0] = 0 if wizard.grab().save(str(path), "PNG") else 1
            except (OSError, ValueError):
                smoke_exit[0] = 1
            wizard.reject()
        wizard.show()
        QTimer.singleShot(300, capture_smoke)
    wizard.exec()
    if options.gui_smoke_test:
        return smoke_exit[0]
    return wizard.exit_code if wizard.success or wizard.currentId() == RESULT else 1


if __name__ == "__main__":
    raise SystemExit(main())
