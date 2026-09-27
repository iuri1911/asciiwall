// Desktop double-clicks for asciiwall. Omarchy's background service opens the
// stock image picker on a left double-click; this catches the click first
// (transparent, input-only, on the Bottom layer above the shell's Background
// layer) and runs `asciiwall pick` instead, which shows the scene carousel, or
// the stock picker while asciiwall is turned off. Right double-click keeps the
// stock theme switcher. Same gestures and commands as
// shell/plugins/background/Background.qml (Omarchy, MIT, (c) David Heinemeier
// Hansson).
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import QtQuick

Item {
  id: root

  Process {
    id: pickProc
    command: ["bash", "-c", "if [[ -x $HOME/.local/bin/asciiwall ]]; then exec \"$HOME/.local/bin/asciiwall\" pick; fi; background=$(omarchy-theme-bg-switcher); [[ -n $background ]] && omarchy-theme-bg-set \"$background\""]
  }

  Process {
    id: themeProc
    command: ["bash", "-c", "theme=$(omarchy-theme-switcher); [[ -n $theme ]] && omarchy-theme-set \"$theme\" >/dev/null 2>&1 &"]
  }

  Variants {
    model: Quickshell.screens

    PanelWindow {
      required property var modelData

      screen: modelData
      anchors { top: true; bottom: true; left: true; right: true }
      color: "transparent"
      WlrLayershell.namespace: "asciiwall-desktop"
      WlrLayershell.layer: WlrLayer.Bottom
      WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
      exclusionMode: ExclusionMode.Ignore

      MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        onDoubleClicked: function(mouse) {
          if (mouse.button === Qt.RightButton) {
            if (!themeProc.running) themeProc.running = true
          } else if (!pickProc.running) {
            pickProc.running = true
          }
          mouse.accepted = true
        }
      }
    }
  }
}
