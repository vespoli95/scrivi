# Repository instructions

## Build, test, and vet

Use Go 1.24 or newer. Fyne builds require a C compiler; on Debian/Ubuntu, install the desktop build dependencies with:

```sh
sudo apt-get install build-essential pkg-config libgl1-mesa-dev xorg-dev
```

Run the app in a graphical desktop session with `go run .`, or build it with `go build -o scrivi .`.

Headless tests use the `ci` build tag:

```sh
go test -tags ci ./...
```

Run one test by name with:

```sh
go test -tags ci -run '^TestTodoLifecycle$' ./...
```

Run static analysis with `go vet ./...`. There is no separate linter configured.

## Architecture

This is a single-package Go/Fyne desktop application. `main.go` creates the app and window, opens the database under the user's config directory (`scrivi/todos.sqlite`), and connects the UI to storage. `ui.go` builds the entry/list controls and refreshes the displayed rows after actions.

`store.go` implements `memoryDB`: an in-memory map is the UI-facing view, while SQLite persists each change immediately and supplies data at startup. A mutation reaches disk before the in-memory map changes, so failed writes preserve the prior view. The database methods are designed for the UI goroutine; they are not a cross-process synchronization mechanism. Run only one app instance at a time.

`updates.go` checks for releases only on Windows release builds (`appVersion` is set from the tag at build time; `dev` skips checks). It validates stable `vMAJOR.MINOR.PATCH` versions and constructs the installer URL from a fixed GitHub path rather than trusting a URL in release metadata. Update detection only opens the download in a browser after user confirmation; it does not install updates.

## Repository conventions

- Keep storage changes durable before updating cached state. Return contextual errors to callers; the UI presents operation errors with Fyne dialogs.
- Storage and update behavior have headless tests in `store_test.go` and `updates_test.go`. Keep those tests runnable with `-tags ci`.
- Windows CI builds with CGO enabled and MinGW, then runs `go test -tags ci ./...` and `go vet ./...` before building the executable. Local `go build` produces the host-platform app.
- A stable `vMAJOR.MINOR.PATCH` tag triggers an unsigned Windows installer and GitHub Release. Version components must be at most 65535, and published versions must increase; do not move or reuse release tags.
- The Inno Setup `AppId` in `packaging/windows/scrivi.iss` must remain unchanged so upgrades target the existing installation. Installer/uninstaller operations must leave the SQLite database in the user's AppData directory untouched.

Commit and push to GitHub after every fix/feature.
