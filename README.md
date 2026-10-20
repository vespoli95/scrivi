# Scrivi

A tiny Go desktop todo list built with Fyne. Add todos with the Add button or
Enter, check them off, and delete them with the trash button.

Todos are kept in memory while the app runs. Each change is saved immediately
to a local SQLite database and loaded back into memory on startup. No server
is needed. If a save fails, Scrivi shows an error and keeps the previous state.

The database is stored in your user configuration directory:

- Linux: `$XDG_CONFIG_HOME/scrivi/todos.sqlite` (default `~/.config/scrivi/todos.sqlite`)
- macOS: `~/Library/Application Support/scrivi/todos.sqlite`
- Windows: `%AppData%\scrivi\todos.sqlite`

Use one running instance of Scrivi at a time; its in-memory view does not
automatically refresh changes from other processes.

## Run

Install Go 1.24 or newer and a C compiler. On Debian/Ubuntu, install the desktop
build dependencies:

```sh
sudo apt-get install build-essential pkg-config libgl1-mesa-dev xorg-dev
```

Then, from this directory:

```sh
go run .
```

A graphical desktop session is required. On macOS, install the Xcode command-line
tools; on Windows, use a GCC toolchain such as MinGW-w64.

## Build

```sh
go build -o scrivi .
```

## Test without a display

```sh
go test -tags ci ./...
```
