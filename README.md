# qexp

A file explorer for the terminal, part of the Quvyta ecosystem. Every kind of file has an icon of its own, a file opens with the program your desktop opens it with, archives are extracted with a right click, and the folder operations you expect are all there: new, rename, cut, copy, paste, drag and drop, and delete into the trash.

The package is `quvyta-explorer`; the commands are `qexp` and `quvyta-explorer`.

```
cargo install quvyta-explorer
```

## Using it

```
qexp [PATH]
```

- A folder opens there.
- A file opens its folder, with the file under the cursor.
- Without a path, qexp opens your home folder.
- A path that does not exist is a one-line message and exit code 2; no screen opens.
- `qexp --version` and `qexp --help` print and leave. Quitting qexp exits with code 0.

The screen has the way back, forward and up, the path (click any part of it to go there), and the view picker at the top; your places on the left (home, the folders your desktop names in `user-dirs.dirs`, and the root); the folder in the middle; and the keys at the bottom. Below 90 columns the places fold away and `ctrl+b` brings them over the folder.

Opening a file uses the desktop's own databases: shared-mime-info tells what the file is, and the desktop entries and `mimeapps.list` tell which program opens it, so a file opens here with the program it opens with everywhere else. A program that runs in the terminal gets the terminal until it ends, then qexp comes back. A program with windows starts beside qexp, and only when there is a graphical session to show it. A text file nothing else opens goes to `$VISUAL`, `$EDITOR` or `less`.

## Keys

| What | Key | Mouse |
|---|---|---|
| Move between rows | ↑ ↓, Home, End, PgUp, PgDn | click selects |
| Open a folder or a file | Enter | double click |
| Open with another program | ctrl+enter | right click, Open with… |
| Up to the folder above | Backspace, alt+↑ | ↑ in the top strip, a part of the path |
| Back, forward | alt+←, alt+→ | ‹ › in the top strip |
| Go to a path | ctrl+l, then type it (`~` is home) | the prompt button in the top strip |
| Places and favourites | alt+1 … alt+9, counting down the sidebar | the places on the left |
| Add a folder to the favourites | the menu key on the folder | right click, Add to favourites |
| Move a favourite up or down | alt+shift+↑ ↓ on it in the sidebar | right click, Move up, Move down |
| Take a favourite out | the menu key on it in the sidebar | right click, Remove from favourites |
| Places on a narrow screen | ctrl+b | the edge button in the top strip |
| Hidden files | ctrl+h or alt+. | settings |
| View: list, grid, tree | ctrl+1, ctrl+2, ctrl+3 | the picker in the top strip |
| Rename | F2 | right click |
| Move to the trash | Delete | right click |
| Delete for good (always asks) | shift+Delete | right click |
| Select several | Space, shift+↑ ↓ | ctrl+click, shift+click, or drag a box from empty space |
| Select all | ctrl+a | |
| Keep only the entry under the cursor | Esc | |
| Cut, copy, paste | ctrl+x, ctrl+c, ctrl+v | right click; or drag onto a folder, holding ctrl to copy; a drop on the folder's own top row goes to the folder above |
| Actions on a row | the menu key, shift+F10 | right click |
| Settings | ctrl+, | the settings button |
| Every key | ? | |
| Quit | q, ctrl+q | |

Many terminals send ctrl+h as Backspace; alt+. shows hidden files in every terminal.

## Favourites

Below the places are your favourites: folders you add with **Add to favourites** on a folder's right-click menu (the shown folder's own top row has it too). They stay in the order you added them until you move them, and a folder that is no longer there is drawn faint; clicking it says so instead of going anywhere. The list is kept one path per line in `~/.local/share/quvyta/explorer/favorites`. The first time qexp starts, before it has kept a list of its own, it takes in the local folders of your desktop's GTK bookmarks (`~/.config/gtk-3.0/bookmarks`, the ones Nautilus and Thunar show), so what you added there is already here. qexp never writes that file, and after that first start the two lists are separate.

## Archives

A right click on an archive offers **Extract here** and **Extract to…**. The archive becomes a new folder named after it (`backup.tar.gz` becomes `backup/`, or `backup 2` when that name is taken); nothing already there is ever replaced, so extracting never asks. The work is done by the tools your system already trusts: `bsdtar` first (it reads zip, tar in every compression, 7z, rar, iso, deb and rpm), then `tar`, `unzip`, 7-Zip, `unrar`, and the compressors for a lone `.gz`, `.xz`, `.zst`, `.bz2`, `.lz4` or `.lz` file. When none is installed, the item says which one is needed; qexp installs nothing. A long extraction runs in the background with a Cancel button, and a cancelled or failed one leaves nothing behind. Encrypted archives are not supported yet.

## Settings

Settings live in `explorer.conf` in the shared Quvyta folder (`~/.config/quvyta/` on Linux): the view, whether hidden files are shown, and whether icons are coloured by their kind (off by default; it needs a terminal with more than 16 colours, and the shape of an icon tells the kind either way). The language, theme and icons are shared by every Quvyta application and can be set for qexp alone. Only what differs from the default is written.

## With qdesk

qexp is made to be qdesk's folder window. The contract is the command line above: qdesk opens a folder by running `qexp <folder>` in a window of its own, and qexp exits with code 0 when it is quit, so the window can close. Inside qdesk (`QDESK=1`) qexp behaves the same: a program it opens in the terminal takes over that window until it ends, then qexp comes back.

When qdesk is installed, a right click on a PNG, JPEG, GIF or WebP picture offers **Set as wallpaper**, which runs `qdesk wallpaper <picture>` in the background and says in the corner whether qdesk took it.

## Network

Once a day at start, qexp asks crates.io whether a newer version of `quvyta-explorer` is out, and says so in a notice when there is one. Only the package's name and version are sent. The question never holds up the start and is silent without a network. It can be turned off on the settings page ("Say when an update is out"); the switch is shared by every Quvyta application.

## Licence

MIT.
