package main

import (
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"net/url"
	"regexp"
	"runtime"
	"strconv"
	"strings"
	"time"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/dialog"
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

func checkForUpdates(a fyne.App, w fyne.Window) {
	if runtime.GOOS != "windows" || appVersion == "dev" {
		return
	}
	go func() {
		client := &http.Client{Timeout: 15 * time.Second}
		version, download, err := findUpdate(client,
			"https://api.github.com/repos/"+releaseRepository+"/releases/latest",
			releaseRepository, appVersion)
		if err != nil {
			log.Printf("Automatic update check failed: %v", err)
			return
		}
		if download == "" {
			return
		}
		fyne.Do(func() {
			dialog.ShowConfirm("Update available",
				fmt.Sprintf("scrivi %s is available (installed: %s).\nOpen your browser to download the unsigned installer?\nClose scrivi and run the downloaded setup to update.", version, appVersion),
				func(accepted bool) {
					if !accepted {
						return
					}
					link, err := url.Parse(download)
					if err != nil {
						dialog.ShowError(err, w)
						return
					}
					if err := a.OpenURL(link); err != nil {
						dialog.ShowError(err, w)
					}
				}, w)
		})
	}()
}
