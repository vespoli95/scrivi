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

## Windows installation and automatic updates

The Windows workflow builds an x64 executable on pull requests and pushes to
`main`. Pushing a stable version tag also creates an **unsigned Inno Setup
installer** and publishes it on GitHub Releases. No signing certificate, PFX,
certificate password, or Azure account is required.

Release builds automatically check GitHub for a newer version at startup on
Windows. If one is available, scrivi offers to open its installer download in
your browser. Close scrivi and run the downloaded installer to upgrade.
Detection is automatic; installation requires your confirmation and is not
silent or unattended. Development builds (`dev`) skip update checks.

### One-time release setup

The repository must be public so users can check releases and download updates
without credentials. Enable GitHub Actions in `vespoli95/scrivi`. The workflow
uses GitHub's built-in token to publish releases; no custom secrets or
`windows-release` environment are needed. Remove the old certificate secrets
if you previously configured them. Protect release tags and limit who can
publish releases: unsigned downloads rely on control of this GitHub repository
and HTTPS, not a verified Windows publisher.

### Publish an update

Commit and push the application and workflow first, then push a new version tag:

```sh
git tag v1.0.0
git push origin v1.0.0
```

Use increasing `vMAJOR.MINOR.PATCH` versions, for example `v1.0.0`. Each component
must be at most 65535. Prerelease suffixes are not supported by this stable
release workflow.
Do not move or reuse published tags. For the next update, use `v1.0.1`.
Keep the latest GitHub Release pointed at the highest stable Windows version.

The release job compiles the installer with Inno Setup 6 on the Windows runner
and refuses to publish a version that does not increase. The build job also
exposes the executable as a workflow artifact for development.

### Install on Windows

Use an x64 Windows 10 or Windows 11 PC. Download
`scrivi-vVERSION-windows-x64-setup.exe` from:

Download and open:

<https://github.com/vespoli95/scrivi/releases/latest>

The installer installs for the current user without administrator privileges,
creates a Start menu shortcut, and offers an optional desktop shortcut.
It includes an uninstaller. Its fixed application ID must stay unchanged across
versions so upgrades use the existing installation.

**Windows SmartScreen or your browser may warn about an unknown publisher or
block an unsigned installer.** Only install downloads from the repository you
trust. Organizational policies may prohibit unsigned apps entirely; this
installer cannot bypass those policies.

SQLite stays at `%AppData%\scrivi\todos.sqlite`, outside the installation
directory. Upgrades and this installer's uninstall process leave that data
alone. If migrating from an MSIX installation, back up and manually migrate its
potentially virtualized database before uninstalling that package.
Offline use is supported; failed update checks are logged and do not block the
app. Checks use GitHub's unauthenticated API, so rate limits may delay detection.

### Windows release acceptance check

Before distributing publicly, install `v1.0.0` using the setup executable on a
clean Windows PC. Check shortcuts, add and complete todos, and close and reopen
scrivi. Publish `v1.0.1`, reopen the installed app, accept its update notification,
and run the downloaded setup. Confirm your todos survive and the notification
does not reappear on the latest version. Check offline launch and uninstall too.
This Windows installation/update check cannot be replaced by headless Go tests.
