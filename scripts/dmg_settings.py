# dmgbuild settings for the macOS installer window (the window you see when opening Oops-mac.dmg).
# dmgbuild writes Finder's window layout itself, so it works on CI machines without a GUI,
# unlike Tauri's built-in DMG step, which skips the styling on CI.
#
#   dmgbuild -s scripts/dmg_settings.py \
#     -D app=path/to/Oops.app -D background=app/src-tauri/dmg/background.tiff \
#     -D icon=app/src-tauri/icons/icon.icns "Oops" Oops-mac.dmg
#
# The background is designed for this exact layout (see design/dmg.html):
# a 640x420 window, 112-pt icons, Oops at (160, 200) and Applications at (480, 200).
import os.path

app = defines["app"]  # noqa: F821  (provided by dmgbuild)
app_name = os.path.basename(app)

format = "ULFO"            # LZFSE: small and fast to open (macOS 10.11+)
filesystem = "HFS+"
files = [app]
symlinks = {"Applications": "/Applications"}
icon = defines.get("icon")  # noqa: F821  volume icon in Finder's sidebar / desktop
background = defines["background"]  # noqa: F821  multi-resolution TIFF (1x + 2x)

window_rect = ((200, 160), (640, 420))
default_view = "icon-view"
show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False
show_icon_preview = False
include_icon_view_settings = True
include_list_view_settings = False
arrange_by = None
label_pos = "bottom"
text_size = 13
icon_size = 112
icon_locations = {
    app_name: (160, 200),
    "Applications": (480, 200),
}
