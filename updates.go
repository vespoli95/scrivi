package main

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"runtime"
	"strconv"
	"strings"
	"time"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/dialog"
	"fyne.io/fyne/v2/widget"
)

var (
	appVersion        = "dev"
	releaseRepository = "vespoli95/scrivi"
	stableVersion     = regexp.MustCompile(`^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$`)
)

func parseVersion(value string) ([3]int, error) {
	var version [3]int
	if !stableVersion.MatchString(value) {
		return version, fmt.Errorf("invalid release version %q", value)
	}
	for i, part := range strings.Split(value[1:], ".") {
		n, err := strconv.Atoi(part)
		if err != nil || n > 65535 {
			return version, fmt.Errorf("invalid release version %q", value)
		}
		version[i] = n
	}
	return version, nil
}

func newerVersion(current, latest string) (bool, error) {
	oldVersion, err := parseVersion(current)
	if err != nil {
		return false, err
	}
	newVersion, err := parseVersion(latest)
	if err != nil {
		return false, err
	}
	for i := range oldVersion {
		if newVersion[i] != oldVersion[i] {
			return newVersion[i] > oldVersion[i], nil
		}
	}
	return false, nil
}

// Return a fixed GitHub download URL, never an arbitrary URL from release metadata.
func findUpdate(client *http.Client, endpoint, repository, current string) (string, string, error) {
	req, err := http.NewRequest(http.MethodGet, endpoint, nil)
	if err != nil {
		return "", "", err
	}
	req.Header.Set("Accept", "application/vnd.github+json")
	req.Header.Set("User-Agent", "scrivi/"+current)
	response, err := client.Do(req)
	if err != nil {
		return "", "", fmt.Errorf("check for updates: %w", err)
	}
	defer response.Body.Close()
	if response.StatusCode == http.StatusNotFound {
		return "", "", nil
	}
	if response.StatusCode != http.StatusOK {
		return "", "", fmt.Errorf("check for updates: GitHub returned HTTP %d", response.StatusCode)
	}
	var release struct {
		TagName    string `json:"tag_name"`
		Draft      bool   `json:"draft"`
		Prerelease bool   `json:"prerelease"`
		Assets     []struct {
			Name string `json:"name"`
		} `json:"assets"`
	}
	if err := json.NewDecoder(io.LimitReader(response.Body, 2<<20)).Decode(&release); err != nil {
		return "", "", fmt.Errorf("read update information: %w", err)
	}
	if release.Draft || release.Prerelease {
		return "", "", nil
	}
	newer, err := newerVersion(current, release.TagName)
	if err != nil {
		return "", "", err
	}
	if !newer {
		return "", "", nil
	}
	filename := "scrivi-" + release.TagName + "-windows-x64-setup.exe"
	for _, asset := range release.Assets {
		if asset.Name == filename {
			return release.TagName, "https://github.com/" + repository + "/releases/download/" + release.TagName + "/" + filename, nil
		}
	}
	return "", "", fmt.Errorf("release %s has no Windows installer", release.TagName)
}

func downloadInstaller(ctx context.Context, client *http.Client, download, dir string) (string, error) {
	checksum, err := updateResponse(ctx, client, download+".sha256")
	if err != nil {
		return "", err
	}
	data, err := io.ReadAll(io.LimitReader(checksum.Body, 1024))
	checksum.Body.Close()
	if err != nil {
		return "", fmt.Errorf("read installer checksum: %w", err)
	}
	expected, err := hex.DecodeString(strings.TrimSpace(string(data)))
	if err != nil || len(expected) != sha256.Size {
		return "", fmt.Errorf("invalid installer checksum")
	}

	response, err := updateResponse(ctx, client, download)
	if err != nil {
		return "", err
	}
	defer response.Body.Close()
	const maxInstallerSize = 128 << 20
	if response.ContentLength > maxInstallerSize {
		return "", fmt.Errorf("installer exceeds the 128 MiB download limit")
	}
	if err := os.MkdirAll(dir, 0700); err != nil {
		return "", fmt.Errorf("create update directory: %w", err)
	}
	file, err := os.CreateTemp(dir, "download-*.exe")
	if err != nil {
		return "", fmt.Errorf("create installer download: %w", err)
	}
	defer os.Remove(file.Name())
	hash := sha256.New()
	size, copyErr := io.Copy(io.MultiWriter(file, hash), io.LimitReader(response.Body, maxInstallerSize+1))
	closeErr := file.Close()
	if copyErr != nil {
		return "", fmt.Errorf("download installer: %w", copyErr)
	}
	if closeErr != nil {
		return "", fmt.Errorf("save installer: %w", closeErr)
	}
	if size > maxInstallerSize {
		return "", fmt.Errorf("installer exceeds the 128 MiB download limit")
	}
	if hex.EncodeToString(hash.Sum(nil)) != hex.EncodeToString(expected) {
		return "", fmt.Errorf("installer checksum does not match; update was not installed")
	}
	path := filepath.Join(dir, "setup.exe")
	if err := os.Rename(file.Name(), path); err != nil {
		return "", fmt.Errorf("save verified installer: %w", err)
	}
	return path, nil
}

func updateResponse(ctx context.Context, client *http.Client, address string) (*http.Response, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, address, nil)
	if err != nil {
		return nil, fmt.Errorf("create update download request: %w", err)
	}
	response, err := client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("download update: %w", err)
	}
	if response.StatusCode != http.StatusOK {
		response.Body.Close()
		return nil, fmt.Errorf("download update: HTTP %d", response.StatusCode)
	}
	return response, nil
}

func checkForUpdates(a fyne.App, w fyne.Window) {
	if runtime.GOOS != "windows" || appVersion == "dev" {
		return
	}
	ctx, cancel := context.WithCancel(context.Background())
	w.SetOnClosed(cancel)
	var declinedVersion string
	go watchUpdates(ctx, 5*time.Minute, func() {
		client := &http.Client{Timeout: 15 * time.Second}
		version, download, err := findUpdate(client,
			"https://api.github.com/repos/"+releaseRepository+"/releases/latest",
			releaseRepository, appVersion)
		if err != nil {
			log.Printf("Automatic update check failed: %v", err)
			return
		}
		if download == "" || version == declinedVersion {
			return
		}
		if offerUpdate(ctx, a, w, version, download) {
			declinedVersion = version
		}
	})
}

func watchUpdates(ctx context.Context, interval time.Duration, check func()) {
	ticker := time.NewTicker(interval)
	defer ticker.Stop()
	for {
		if ctx.Err() != nil {
			return
		}
		check()
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
		}
	}
}

func offerUpdate(ctx context.Context, a fyne.App, w fyne.Window, version, download string) bool {
	accepted := make(chan bool, 1)
	fyne.Do(func() {
		if ctx.Err() != nil {
			return
		}
		dialog.ShowConfirm("Update available",
			fmt.Sprintf("scrivi %s is available (installed: %s).\nDownload and run the unsigned installer now?\nScrivi will restart. Your todos will be kept.", version, appVersion),
			func(ok bool) { accepted <- ok }, w)
	})
	select {
	case <-ctx.Done():
		return false
	case ok := <-accepted:
		if !ok {
			return true
		}
	}

	var progress dialog.Dialog
	fyne.DoAndWait(func() {
		progress = dialog.NewCustomWithoutButtons("Downloading update", widget.NewProgressBarInfinite(), w)
		progress.Show()
	})
	err := installUpdate(ctx, download)
	fyne.Do(func() {
		progress.Hide()
		if ctx.Err() != nil {
			return
		}
		if err != nil {
			dialog.ShowError(err, w)
			return
		}
		a.Quit()
	})

	// Keep the watcher from offering another update before the app exits.
	if err == nil {
		<-ctx.Done()
	}
	return false
}

func installUpdate(ctx context.Context, download string) error {
	cache, err := os.UserCacheDir()
	if err != nil {
		return fmt.Errorf("find update cache: %w", err)
	}
	client := &http.Client{Timeout: 5 * time.Minute}
	path, err := downloadInstaller(ctx, client, download, filepath.Join(cache, "scrivi", "updates"))
	if err != nil {
		return err
	}
	if err := ctx.Err(); err != nil {
		return err
	}
	command := exec.Command(path, "/SILENT", "/NORESTART", "/SP-", "/RESTARTSCRIVI=1")
	if err := command.Start(); err != nil {
		return fmt.Errorf("start update installer: %w", err)
	}
	if err := command.Process.Release(); err != nil {
		log.Printf("Release installer process: %v", err)
	}
	return nil
}
