# Changelog

Every release of quvyta-explorer, newest first. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## 0.1.1 - 2026-09-24

### Added

- Favourites below the places: **Add to favourites** on a folder's menu, **Move up**, **Move down** and **Remove from favourites** on a favourite's own menu, alt+shift with the arrows to move one from the keys, and alt+1 … alt+9 counting them after the places. A favourite whose folder is gone is drawn faint. They are kept in `~/.local/share/quvyta/explorer/favorites`.
- On the first start, the local folders of the desktop's GTK bookmarks are taken in as the first favourites. qexp never writes the GTK file.
- The `?` overview names ctrl+a, Esc and the keys that move a favourite, and calls alt with a number a sidebar entry.

### Changed

- Built on quvyta-framework 0.1.28, whose trees leave no room for an arrow no row has, so the favourites line up with the places.

## 0.1.0 - 2026-09-24

The first release.

### Added

- A file explorer for the terminal: back, forward and up, a path you can click or type, your places on the left, and a list, grid or tree view.
- An icon for every kind of file, with a shape of its own for each family when there is no Nerd Font, and colours by kind as an option.
- Files open with the program your desktop opens them with, read from shared-mime-info, the desktop entries and `mimeapps.list`; **Open with…** lists the others.
- The mouse works like a desktop file explorer: a click selects, a double click opens, a drag onto a folder moves (with ctrl, copies), a drop on the folder's own top row goes to the folder above, and a box drawn on empty space selects. From the keys, shift with the arrows selects several, ctrl+a selects all and Esc keeps only the entry under the cursor.
- **Extract here** and **Extract to…** for archives, with the tools the system already has, into a new folder, never over anything.
- Delete moves to the trash; deleting for good always asks.
- ctrl+x, ctrl+c and ctrl+v cut, copy and paste files, as in a desktop file explorer.
- `qexp PATH` for qdesk and other launchers.
- **Set as wallpaper** on a picture's menu when qdesk is installed, through `qdesk wallpaper`.
- English, Turkish, German, Spanish, French, Japanese, Brazilian Portuguese, Russian and Simplified Chinese.
- A notice when a newer version is out, checked once a day and turned off in the settings.
- Built on quvyta-framework 0.1.27.
