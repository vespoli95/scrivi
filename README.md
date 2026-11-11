# Scrivi

A tiny Go desktop todo list built with Fyne. Add todos with the Add button or
Enter, check them off, and delete them with the trash button.

Create categories, such as Life, Money, or Work, using **Add category** or Enter.
Each category has its own todo entry and scrollable list. Use a todo's edit
button to change its text. Deleting a category also deletes its todos after
confirmation. Categories and todos are saved across restarts. Older unassigned
todos, if any, are preserved in a regular **Recovered todos** category.

On Windows, use the **Pin** button to keep Scrivi above other windows.
**Settings** lets you choose whether Scrivi starts when you sign in. The startup
choice persists; pinning lasts until you unpin the window or close Scrivi.
Windows may also allow startup apps to be disabled in its own settings.

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
tools; on Windows, use a GCC toolchain such as MinGW-w64. To embed the Windows
executable icon when building locally, first run from the project root:

```sh
windres -i packaging/windows/icon.rc -o resource_windows_amd64.syso -O coff
```

## Build

```sh
go build -o scrivi .
```

## Test without a display

```sh
go test -tags ci ./...
```

## Windows installation and automatic updates

The Windows workflow builds an x64 executable on pull requests. Each successful
push build on `main` also creates an **unsigned Inno Setup installer** and
publishes a new GitHub Release, automatically incrementing the patch version
(starting at `v1.0.0` if there are no releases). No signing certificate, PFX,
certificate password, or Azure account is required.

Release builds check GitHub at startup and every five minutes while running on
Windows. When a newer version is available, one confirmation downloads the
installer, verifies its SHA-256 checksum, and starts the upgrade. Scrivi then
closes and the installer relaunches it when installation finishes. Todos are
preserved. The installer shows progress and may show Windows security prompts;
updates are not installed without your confirmation. Declining a version
suppresses further prompts for that version until the app is reopened.
Development builds (`dev`) skip update checks.

### One-time release setup

The repository must be public so users can check releases and download updates
without credentials. Enable GitHub Actions in `vespoli95/scrivi`. The workflow
uses GitHub's built-in token to publish releases; no custom secrets or
`windows-release` environment are needed. Remove the old certificate secrets
if you previously configured them. Protect release tags and limit who can
publish releases: unsigned downloads rely on control of this GitHub repository
and HTTPS, not a verified Windows publisher.

### Publish an update

Commit and push to `main`. Once the Windows build succeeds, GitHub Actions
publishes an installer, its `.exe.sha256` checksum, and a version tag pointing at
that commit. There is no need to create a tag for routine updates. Pushes made
while a release workflow is running may be coalesced into the newest pending
push; the newest build includes the preceding changes.

To choose a specific version (for example, a major or minor release), push a
stable version tag instead:

```sh
git tag v2.0.0
git push origin v2.0.0
```

Use an unused `vMAJOR.MINOR.PATCH` version higher than the latest release. Each
component must be at most 65535. Prerelease suffixes are not supported by this
stable release workflow.
Do not move or reuse published tags. Automatic updates increment the patch
component, carrying into the minor and major components if necessary.
Keep the latest GitHub Release pointed at the highest stable Windows version.

The release job compiles the installer with Inno Setup 6 on the Windows runner
and refuses to publish a version that does not increase. Release workflows are
serialized so concurrent builds do not choose the same automatic version. The
build job also exposes the executable as a workflow artifact for development.

### Install on Windows

Use an x64 Windows 10 or Windows 11 PC. Download
`scrivi-vVERSION-windows-x64-setup.exe` from:

Download and open:

<https://github.com/vespoli95/scrivi/releases/latest>

The installer installs for the current user without administrator privileges,
creates Start menu and desktop shortcuts by default, and lets you deselect the
desktop shortcut during installation.
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
Verified installers are cached at `%LocalAppData%\scrivi\updates\setup.exe`;
failed or incomplete downloads are discarded. Checksums detect damaged downloads,
not a compromised publisher: installers remain unsigned.

Existing installations with the older browser-only updater need to install
the first release containing this updater once. Later updates use the in-app
download and restart flow.

### Windows release acceptance check

Before distributing publicly, install `v1.0.0` using the setup executable on a
clean Windows PC. Check shortcuts, add and complete todos, and close and reopen
scrivi. Push a change to `main`, keep the app open, and accept its update
notification after the new release is published. Confirm the installer runs,
scrivi restarts, your todos survive, and the notification does not reappear on
the latest version. Check offline launch and uninstall too.
This Windows installation/update check cannot be replaced by headless Go tests.
